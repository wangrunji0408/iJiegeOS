/// 网络栈
/// 使用 smoltcp 实现 TCP/IP

use smoltcp::{
    iface::{Config, Interface, SocketSet},
    socket::{tcp, udp},
    time::Instant,
    wire::{EthernetAddress, IpAddress, IpCidr, Ipv4Address},
    phy::{Device, DeviceCapabilities, Medium, RxToken, TxToken},
};
use alloc::vec::Vec;
use alloc::collections::BTreeMap;
use spin::Mutex;
use lazy_static::lazy_static;

use crate::fs::socket::{Socket, SockAddr};

/// 网络接口状态
pub struct NetInterface {
    pub iface: Interface,
    pub sockets: SocketSet<'static>,
    pub device: VirtioNet,
}

/// VirtIO 网卡驱动（封装）
pub struct VirtioNet {
    tx_buf: alloc::collections::VecDeque<Vec<u8>>,
    rx_buf: alloc::collections::VecDeque<Vec<u8>>,
}

impl VirtioNet {
    pub fn new() -> Self {
        Self {
            tx_buf: alloc::collections::VecDeque::new(),
            rx_buf: alloc::collections::VecDeque::new(),
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
    buf: &'a mut alloc::collections::VecDeque<Vec<u8>>,
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

lazy_static! {
    pub static ref NET_IFACE: Mutex<Option<NetInterface>> = Mutex::new(None);
}

/// 已绑定的本地端口 -> socket
static BOUND_SOCKETS: Mutex<BTreeMap<u16, usize>> = Mutex::new(BTreeMap::new());

pub fn init() {
    // 初始化网络接口（由 VirtIO 驱动初始化后调用）
    log::info!("net: network stack initialized");
}

/// 驱动初始化后设置网络接口
pub fn setup_interface(mac: [u8; 6]) {
    let device = VirtioNet::new();

    let config = Config::new(EthernetAddress(mac).into());
    let mut iface = Interface::new(config, &mut { VirtioNet::new() }, smoltcp_now());

    // 配置 IP 地址
    iface.update_ip_addrs(|addrs| {
        addrs.push(IpCidr::new(IpAddress::v4(10, 0, 2, 15), 24)).unwrap();
    });

    // 设置默认网关
    iface.routes_mut().add_default_ipv4_route(Ipv4Address::new(10, 0, 2, 2)).unwrap();

    let sockets = SocketSet::new(Vec::new());

    log::info!("net: interface configured: 10.0.2.15/24, gw=10.0.2.2, mac={:?}", mac);
}

fn smoltcp_now() -> Instant {
    Instant::from_millis(crate::timer::get_time_ms() as i64)
}

/// 轮询网络接口
pub fn poll() {
    let mut guard = NET_IFACE.lock();
    if let Some(iface) = guard.as_mut() {
        let timestamp = smoltcp_now();
        iface.iface.poll(timestamp, &mut iface.device, &mut iface.sockets);
    }
}

/// Socket 系统调用实现
pub fn socket_create(domain: i32, sock_type: i32, protocol: i32) -> i64 {
    // 创建 socket 并注册
    0
}

pub fn socket_bind(sock: &Socket, addr: &[u8]) -> i64 {
    0
}

pub fn socket_listen(sock: &Socket, backlog: i32) -> i64 {
    0
}

pub fn socket_accept(sock: &Socket) -> Option<(alloc::sync::Arc<Socket>, SockAddr)> {
    None
}

pub fn socket_connect(sock: &Socket, addr: &SockAddr) -> i64 {
    0
}

pub fn socket_send(sock: &Socket, buf: &[u8], flags: i32) -> isize {
    // 通过 smoltcp 发送数据
    let mut inner = sock.inner.lock();
    if !inner.connected { return -111; }  // ECONNREFUSED

    // 简化：直接写入发送缓冲区
    for &b in buf {
        inner.send_buf.push_back(b);
    }

    // 触发网络轮询
    drop(inner);
    poll();

    buf.len() as isize
}

pub fn socket_recv(sock: &Socket, buf: &mut [u8], flags: i32) -> isize {
    // 先轮询网络
    poll();

    let mut inner = sock.inner.lock();
    if inner.recv_buf.is_empty() {
        if inner.nonblock {
            return -11;  // EAGAIN
        }
        return 0;
    }

    let n = buf.len().min(inner.recv_buf.len());
    for i in 0..n {
        buf[i] = inner.recv_buf.pop_front().unwrap();
    }
    n as isize
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
