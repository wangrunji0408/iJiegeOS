/// 网络栈
/// 使用 smoltcp 实现 TCP/IP

use smoltcp::{
    iface::{Config, Interface, SocketSet},
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

/// VirtIO 网卡驱动（封装）
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
    /// 端口 -> (smoltcp handle, nginx socket ptr)
    pub tcp_listeners: BTreeMap<u16, smoltcp::iface::SocketHandle>,
    /// 端口 -> nginx 监听 socket 的接受队列（已建立连接的 smoltcp handle）
    pub pending_accepts: BTreeMap<u16, VecDeque<smoltcp::iface::SocketHandle>>,
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
}

fn smoltcp_now() -> Instant {
    Instant::from_millis(crate::timer::get_time_ms() as i64)
}

/// 轮询网络接口：从 VirtIO 设备读取数据 → smoltcp → 写回 VirtIO
pub fn poll() {
    // 先从真实 VirtIO 设备读取数据包，推入 smoltcp device 的 rx_buf
    if let Some(ref mut net_dev) = crate::drivers::virtio::NET_DEVICE.lock().as_mut() {
        if let Ok(rx_buf) = net_dev.receive() {
            let pkt = rx_buf.packet().to_vec();
            drop(rx_buf);
            if let Some(ref mut iface_state) = NET_IFACE.lock().as_mut() {
                iface_state.device.push_rx(pkt);
            }
        }
    }

    // 运行 smoltcp 的网络栈处理
    let mut guard = NET_IFACE.lock();
    if let Some(ref mut state) = guard.as_mut() {
        let timestamp = smoltcp_now();
        state.iface.poll(timestamp, &mut state.device, &mut state.sockets);

        // 把 smoltcp 想发出的数据包发给 VirtIO 设备
        while let Some(pkt) = state.device.pop_tx() {
            drop(guard);
            if let Some(ref mut net_dev) = crate::drivers::virtio::NET_DEVICE.lock().as_mut() {
                let _ = net_dev.send(pkt.len(), |buf| {
                    let n = pkt.len().min(buf.len());
                    buf[..n].copy_from_slice(&pkt[..n]);
                });
            }
            guard = NET_IFACE.lock();
            if guard.is_none() { break; }
        }

        // 检查监听 TCP socket 是否有新连接
        if let Some(ref mut state) = guard.as_mut() {
            let mut new_conns: Vec<(u16, smoltcp::iface::SocketHandle)> = Vec::new();
            for (&port, &listener_handle) in state.tcp_listeners.iter() {
                // 检查是否有新连接，如果是，创建新的监听 socket
                let tcp_sock = state.sockets.get_mut::<tcp::Socket>(listener_handle);
                if tcp_sock.is_active() && tcp_sock.may_recv() {
                    // 这个 socket 已经建立连接，移到 pending_accepts
                    new_conns.push((port, listener_handle));
                }
            }
            for (port, handle) in new_conns {
                state.tcp_listeners.remove(&port);
                state.pending_accepts.entry(port).or_default().push_back(handle);
                // 创建新的监听 socket
                let mut new_tcp = tcp::Socket::new(
                    tcp::SocketBuffer::new(vec![0u8; 65536]),
                    tcp::SocketBuffer::new(vec![0u8; 65536]),
                );
                new_tcp.listen(port).ok();
                let new_handle = state.sockets.add(new_tcp);
                state.tcp_listeners.insert(port, new_handle);
            }
        }
    }
}

/// 绑定端口并开始监听（被 sys_listen 调用）
pub fn tcp_listen(port: u16) {
    let mut guard = NET_IFACE.lock();
    if let Some(ref mut state) = guard.as_mut() {
        // 创建 smoltcp TCP socket 并监听
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

/// 检查端口是否有待接受的连接
pub fn tcp_has_pending(port: u16) -> bool {
    let guard = NET_IFACE.lock();
    if let Some(ref state) = guard.as_ref() {
        state.pending_accepts.get(&port)
            .map(|q| !q.is_empty())
            .unwrap_or(false)
    } else {
        false
    }
}

/// 接受一个连接，返回连接的 smoltcp handle
pub fn tcp_accept(port: u16) -> Option<smoltcp::iface::SocketHandle> {
    let mut guard = NET_IFACE.lock();
    if let Some(ref mut state) = guard.as_mut() {
        state.pending_accepts.get_mut(&port)?.pop_front()
    } else {
        None
    }
}

/// 从 smoltcp socket 读取数据
pub fn tcp_recv(handle: smoltcp::iface::SocketHandle, buf: &mut [u8]) -> isize {
    let mut guard = NET_IFACE.lock();
    if let Some(ref mut state) = guard.as_mut() {
        let tcp_sock = state.sockets.get_mut::<tcp::Socket>(handle);
        if tcp_sock.can_recv() {
            match tcp_sock.recv_slice(buf) {
                Ok(n) => n as isize,
                Err(_) => -1,
            }
        } else if tcp_sock.is_active() {
            0  // 还没数据，稍后重试
        } else {
            -1  // 连接已关闭
        }
    } else {
        -1
    }
}

/// 向 smoltcp socket 发送数据
pub fn tcp_send(handle: smoltcp::iface::SocketHandle, buf: &[u8]) -> isize {
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

/// Socket 系统调用实现（简化版，不使用 smoltcp）
pub fn socket_send(sock: &Socket, buf: &[u8], flags: i32) -> isize {
    let inner = sock.inner.lock();
    if let Some(handle) = inner.handle {
        drop(inner);
        let h = smoltcp::iface::SocketHandle::from(handle);
        tcp_send(h, buf)
    } else {
        drop(inner);
        poll();
        buf.len() as isize  // 假装成功
    }
}

pub fn socket_recv(sock: &Socket, buf: &mut [u8], flags: i32) -> isize {
    poll();

    let inner = sock.inner.lock();
    if let Some(handle) = inner.handle {
        drop(inner);
        let h = smoltcp::iface::SocketHandle::from(handle);
        tcp_recv(h, buf)
    } else {
        drop(inner);
        let mut inner2 = sock.inner.lock();
        if inner2.recv_buf.is_empty() {
            if inner2.nonblock {
                return -11;  // EAGAIN
            }
            return 0;
        }
        let n = buf.len().min(inner2.recv_buf.len());
        for i in 0..n {
            buf[i] = inner2.recv_buf.pop_front().unwrap();
        }
        n as isize
    }
}

/// 获取套接字地址结构
pub fn parse_sockaddr(addr: &[u8]) -> Option<SockAddr> {
    if addr.len() < 8 { return None; }
    // sockaddr_in: sa_family(2) + sin_port(2) + sin_addr(4) + ...
    let family = u16::from_le_bytes([addr[0], addr[1]]);
    if family != 2 { return None; }  // AF_INET
    let port = u16::from_be_bytes([addr[2], addr[3]]);
    let ip = [addr[4], addr[5], addr[6], addr[7]];
    Some(SockAddr { port, ip })
}
