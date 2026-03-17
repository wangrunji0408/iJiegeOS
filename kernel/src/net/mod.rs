/// 网络栈
/// 使用 smoltcp 实现 TCP/IP

use smoltcp::{
    iface::{Config, Interface, SocketHandle, SocketSet},
    socket::tcp,
    time::Instant,
    wire::{EthernetAddress, IpAddress, IpCidr, Ipv4Address},
    phy::{Device, DeviceCapabilities, Medium, RxToken, TxToken},
};
use alloc::vec::Vec;
use alloc::vec;
use alloc::collections::{BTreeMap, VecDeque};
use spin::Mutex;
use lazy_static::lazy_static;

use crate::fs::socket::{Socket, SockAddr};

/// VirtIO 网卡驱动（封装，作为 smoltcp Device）
pub struct VirtioNet {
    tx_buf: VecDeque<Vec<u8>>,
    rx_buf: VecDeque<Vec<u8>>,
}

impl VirtioNet {
    pub fn new() -> Self {
        Self {
            tx_buf: VecDeque::new(),
            rx_buf: VecDeque::new(),
        }
    }

    pub fn push_rx(&mut self, data: Vec<u8>) {
        self.rx_buf.push_back(data);
    }

    pub fn pop_tx(&mut self) -> Option<Vec<u8>> {
        self.tx_buf.pop_front()
    }
}

impl Device for VirtioNet {
    type RxToken<'a> = VirtioRxToken where Self: 'a;
    type TxToken<'a> = VirtioTxToken<'a> where Self: 'a;

    fn receive(&mut self, _timestamp: Instant) -> Option<(Self::RxToken<'_>, Self::TxToken<'_>)> {
        if let Some(data) = self.rx_buf.pop_front() {
            Some((
                VirtioRxToken { data },
                VirtioTxToken { buf: &mut self.tx_buf },
            ))
        } else {
            None
        }
    }

    fn transmit(&mut self, _timestamp: Instant) -> Option<Self::TxToken<'_>> {
        Some(VirtioTxToken { buf: &mut self.tx_buf })
    }

    fn capabilities(&self) -> DeviceCapabilities {
        let mut caps = DeviceCapabilities::default();
        caps.medium = Medium::Ethernet;
        caps.max_transmission_unit = 1514;
        caps.max_burst_size = Some(1);
        caps
    }
}

pub struct VirtioRxToken {
    data: Vec<u8>,
}

impl RxToken for VirtioRxToken {
    fn consume<R, F>(mut self, f: F) -> R
    where F: FnOnce(&mut [u8]) -> R {
        f(&mut self.data)
    }
}

pub struct VirtioTxToken<'a> {
    buf: &'a mut VecDeque<Vec<u8>>,
}

impl<'a> TxToken for VirtioTxToken<'a> {
    fn consume<R, F>(self, len: usize, f: F) -> R
    where F: FnOnce(&mut [u8]) -> R {
        let mut data = vec![0u8; len];
        let r = f(&mut data);
        self.buf.push_back(data);
        r
    }
}

/// 网络接口状态
pub struct NetInterface {
    pub iface: Interface,
    pub sockets: SocketSet<'static>,
    pub device: VirtioNet,
    /// 端口 -> smoltcp TCP socket handle（监听者）
    pub tcp_listeners: BTreeMap<u16, SocketHandle>,
    /// 端口 -> 已建立连接待 accept 的 smoltcp handle 队列
    pub pending_accepts: BTreeMap<u16, VecDeque<SocketHandle>>,
}

lazy_static! {
    pub static ref NET_IFACE: Mutex<Option<NetInterface>> = Mutex::new(None);
}

pub fn init() {
    log::info!("net: network stack initialized");
}

/// 驱动初始化后设置网络接口
pub fn setup_interface(mac: [u8; 6]) {
    let mut device = VirtioNet::new();

    let config = Config::new(EthernetAddress(mac).into());
    let mut iface = Interface::new(config, &mut device, smoltcp_now());

    // 配置 IP 地址：QEMU user-mode 网络的 VM 地址
    iface.update_ip_addrs(|addrs| {
        addrs.push(IpCidr::new(IpAddress::v4(10, 0, 2, 15), 24)).unwrap();
    });

    // 设置默认网关
    iface.routes_mut().add_default_ipv4_route(Ipv4Address::new(10, 0, 2, 2)).unwrap();

    let sockets = SocketSet::new(Vec::new());

    *NET_IFACE.lock() = Some(NetInterface {
        iface,
        sockets,
        device,
        tcp_listeners: BTreeMap::new(),
        pending_accepts: BTreeMap::new(),
    });

    log::info!("net: interface configured: 10.0.2.15/24, gw=10.0.2.2, mac={:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
        mac[0], mac[1], mac[2], mac[3], mac[4], mac[5]);

    // 发送 GARP（Gratuitous ARP）让 QEMU slirp 学习 VM 的 MAC 地址
    // ARP 以太网帧：目标 broadcast, 源 VM MAC
    let garp = make_garp_packet(&mac, &[10, 0, 2, 15]);
    crate::drivers::net_send_packet(&garp);
    log::info!("net: sent GARP packet ({} bytes)", garp.len());
}

fn smoltcp_now() -> Instant {
    Instant::from_millis(crate::timer::get_time_ms() as i64)
}

/// 轮询网络接口：读取 VirtIO RX → smoltcp → 写出 VirtIO TX
pub fn poll() {
    // 从真实 VirtIO 设备接收数据包，推入 smoltcp device rx_buf
    let mut rx_count = 0;
    while let Some(pkt) = crate::drivers::net_receive_packet() {
        rx_count += 1;
        let mut guard = NET_IFACE.lock();
        if let Some(ref mut state) = guard.as_mut() {
            state.device.push_rx(pkt);
        }
    }
    if rx_count > 0 {
        log::debug!("net::poll: received {} packets from VirtIO", rx_count);
    }

    // 运行 smoltcp 网络栈
    {
        let mut guard = NET_IFACE.lock();
        if let Some(ref mut state) = guard.as_mut() {
            let timestamp = smoltcp_now();
            let changed = state.iface.poll(timestamp, &mut state.device, &mut state.sockets);
            if changed {
                log::warn!("net::poll: smoltcp state changed, tx_buf.len={}", state.device.tx_buf.len());
            }
        }
    }

    // 把 smoltcp 要发出的数据包发给 VirtIO
    loop {
        let pkt = {
            let mut guard = NET_IFACE.lock();
            guard.as_mut().and_then(|s| s.device.pop_tx())
        };
        match pkt {
            Some(p) => crate::drivers::net_send_packet(&p),
            None => break,
        }
    }

    // 检查监听 socket 是否有新连接
    let mut guard = NET_IFACE.lock();
    if let Some(ref mut state) = guard.as_mut() {
        let ports: Vec<u16> = state.tcp_listeners.keys().copied().collect();
        for port in ports {
            let handle = state.tcp_listeners[&port];
            let is_connected = {
                let tcp_sock = state.sockets.get::<tcp::Socket>(handle);
                tcp_sock.is_active() && tcp_sock.may_recv()
            };
            if is_connected {
                    // 此连接已建立，移到 pending_accepts
                    state.tcp_listeners.remove(&port);
                    state.pending_accepts.entry(port).or_default().push_back(handle);
                    // 创建新的监听 socket 继续监听
                    let mut new_tcp = tcp::Socket::new(
                        tcp::SocketBuffer::new(vec![0u8; 65536]),
                        tcp::SocketBuffer::new(vec![0u8; 65536]),
                    );
                    if new_tcp.listen(port).is_ok() {
                        let new_handle = state.sockets.add(new_tcp);
                        state.tcp_listeners.insert(port, new_handle);
                        log::warn!("net: new connection on port {}, created new listener", port);
                    }
                }
        }
    }
}

/// 绑定并监听 TCP 端口（被 sys_listen 调用）
pub fn tcp_listen(port: u16) {
    let mut guard = NET_IFACE.lock();
    if let Some(ref mut state) = guard.as_mut() {
        if state.tcp_listeners.contains_key(&port) {
            return;  // 已在监听
        }
        let mut tcp_sock = tcp::Socket::new(
            tcp::SocketBuffer::new(vec![0u8; 65536]),
            tcp::SocketBuffer::new(vec![0u8; 65536]),
        );
        if tcp_sock.listen(port).is_ok() {
            let handle = state.sockets.add(tcp_sock);
            state.tcp_listeners.insert(port, handle);
            log::info!("net: TCP listening on port {}", port);
        } else {
            log::warn!("net: failed to listen on port {}", port);
        }
    }
}

/// 检查是否有待 accept 的连接
pub fn tcp_has_pending(port: u16) -> bool {
    let guard = NET_IFACE.lock();
    guard.as_ref()
        .and_then(|s| s.pending_accepts.get(&port))
        .map(|q| !q.is_empty())
        .unwrap_or(false)
}

/// 从 smoltcp 中接受一个连接
pub fn tcp_accept(port: u16) -> Option<SocketHandle> {
    let mut guard = NET_IFACE.lock();
    guard.as_mut()
        .and_then(|s| s.pending_accepts.get_mut(&port))
        .and_then(|q| q.pop_front())
}

/// 从 smoltcp socket 读取数据
pub fn tcp_recv(handle: SocketHandle, buf: &mut [u8]) -> isize {
    let mut guard = NET_IFACE.lock();
    if let Some(ref mut state) = guard.as_mut() {
        let tcp_sock = state.sockets.get_mut::<tcp::Socket>(handle);
        if tcp_sock.can_recv() {
            match tcp_sock.recv_slice(buf) {
                Ok(n) => n as isize,
                Err(_) => -1,
            }
        } else if tcp_sock.is_active() {
            0  // 无数据，稍后重试
        } else {
            -1  // 连接关闭
        }
    } else {
        -1
    }
}

/// 向 smoltcp socket 写入数据
pub fn tcp_send(handle: SocketHandle, buf: &[u8]) -> isize {
    let mut guard = NET_IFACE.lock();
    if let Some(ref mut state) = guard.as_mut() {
        let tcp_sock = state.sockets.get_mut::<tcp::Socket>(handle);
        if tcp_sock.can_send() {
            match tcp_sock.send_slice(buf) {
                Ok(n) => n as isize,
                Err(_) => -1,
            }
        } else {
            -1
        }
    } else {
        -1
    }
}

/// Socket recv（由 FileDescriptor::read 调用）
pub fn socket_recv(sock: &Socket, buf: &mut [u8], flags: i32) -> isize {
    poll();

    let inner = sock.inner.lock();
    // 如果有 smoltcp handle，从 smoltcp 读
    if let Some(raw_handle) = inner.handle {
        let handle: SocketHandle = unsafe { core::mem::transmute(raw_handle) };
        drop(inner);
        return tcp_recv(handle, buf);
    }

    if inner.recv_buf.is_empty() {
        if inner.nonblock { return -11; }  // EAGAIN
        return 0;
    }
    drop(inner);

    let mut inner2 = sock.inner.lock();
    let n = buf.len().min(inner2.recv_buf.len());
    for i in 0..n {
        buf[i] = inner2.recv_buf.pop_front().unwrap();
    }
    n as isize
}

/// Socket send（由 FileDescriptor::write 调用）
pub fn socket_send(sock: &Socket, buf: &[u8], flags: i32) -> isize {
    let inner = sock.inner.lock();
    // 如果有 smoltcp handle，通过 smoltcp 发送
    if let Some(raw_handle) = inner.handle {
        let handle: SocketHandle = unsafe { core::mem::transmute(raw_handle) };
        drop(inner);
        let n = tcp_send(handle, buf);
        if n > 0 { poll(); }
        return n;
    }

    if !inner.connected { return -111; }  // ECONNREFUSED
    drop(inner);

    let mut inner2 = sock.inner.lock();
    for &b in buf {
        inner2.send_buf.push_back(b);
    }
    drop(inner2);
    poll();
    buf.len() as isize
}

/// 解析 sockaddr_in 结构
pub fn parse_sockaddr(addr: &[u8]) -> Option<SockAddr> {
    if addr.len() < 8 { return None; }
    let family = u16::from_le_bytes([addr[0], addr[1]]);
    if family != 2 { return None; }  // AF_INET only
    let port = u16::from_be_bytes([addr[2], addr[3]]);
    let ip = [addr[4], addr[5], addr[6], addr[7]];
    Some(SockAddr { port, ip })
}
