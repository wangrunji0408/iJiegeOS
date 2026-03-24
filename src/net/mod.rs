mod socket;

pub use socket::*;

use alloc::collections::BTreeMap;
use alloc::sync::Arc;
use alloc::vec;
use alloc::vec::Vec;
use spin::Mutex;
use lazy_static::lazy_static;

use smoltcp::iface::{Config, Interface, SocketSet, SocketHandle as SmolSocketHandle};
use smoltcp::socket::tcp::{Socket as TcpSocket, SocketBuffer};
use smoltcp::wire::{EthernetAddress, IpCidr, Ipv4Address, IpAddress, IpEndpoint};
use smoltcp::phy::{Device, DeviceCapabilities, Medium, RxToken, TxToken};
use smoltcp::time::Instant;

lazy_static! {
    pub static ref NET_STACK: Mutex<Option<NetStack>> = Mutex::new(None);
    static ref NEXT_SOCKFD: Mutex<usize> = Mutex::new(100);
    static ref TCP_LISTEN_HANDLE: Mutex<Option<SmolSocketHandle>> = Mutex::new(None);
}

pub struct NetStack {
    pub iface: Interface,
    pub sockets: SocketSet<'static>,
    pub device: VirtioSmolDevice,
}

/// Adapter between virtio-net driver and smoltcp Device trait
pub struct VirtioSmolDevice;

pub struct VirtioRxToken(Vec<u8>);
pub struct VirtioTxToken;

impl RxToken for VirtioRxToken {
    fn consume<R, F>(mut self, f: F) -> R where F: FnOnce(&mut [u8]) -> R {
        f(&mut self.0)
    }
}

impl TxToken for VirtioTxToken {
    fn consume<R, F>(self, len: usize, f: F) -> R where F: FnOnce(&mut [u8]) -> R {
        let mut buf = vec![0u8; len];
        let result = f(&mut buf);
        crate::drivers::virtio_net::send(&buf);
        result
    }
}

impl Device for VirtioSmolDevice {
    type RxToken<'a> = VirtioRxToken;
    type TxToken<'a> = VirtioTxToken;

    fn receive(&mut self, _timestamp: Instant) -> Option<(Self::RxToken<'_>, Self::TxToken<'_>)> {
        if let Some(data) = crate::drivers::virtio_net::recv() {
            Some((VirtioRxToken(data), VirtioTxToken))
        } else {
            None
        }
    }

    fn transmit(&mut self, _timestamp: Instant) -> Option<Self::TxToken<'_>> {
        Some(VirtioTxToken)
    }

    fn capabilities(&self) -> DeviceCapabilities {
        let mut caps = DeviceCapabilities::default();
        caps.medium = Medium::Ethernet;
        caps.max_transmission_unit = 1514;
        caps
    }
}

pub fn init() {
    // Check if virtio-net is available
    if crate::drivers::virtio_net::VIRTIO_NET.lock().is_none() {
        println!("[NET] No virtio-net device found, socket-only mode");
        return;
    }

    let mac = crate::drivers::virtio_net::mac_address();
    let config = Config::new(EthernetAddress(mac).into());
    let mut device = VirtioSmolDevice;
    let mut iface = Interface::new(config, &mut device, Instant::ZERO);

    iface.update_ip_addrs(|addrs| {
        addrs.push(IpCidr::new(IpAddress::v4(10, 0, 2, 15), 24)).unwrap();
    });
    iface.routes_mut().add_default_ipv4_route(Ipv4Address::new(10, 0, 2, 2)).unwrap();

    let sockets = SocketSet::new(vec![]);

    *NET_STACK.lock() = Some(NetStack {
        iface,
        sockets,
        device,
    });

    println!("[NET] Network stack initialized: 10.0.2.15/24, gw 10.0.2.2");
}

fn get_time_ms() -> i64 {
    let time = riscv::register::time::read() as u64;
    (time * 1000 / crate::config::CLOCK_FREQ as u64) as i64
}

pub fn poll_net() {
    if let Some(ref mut stack) = *NET_STACK.lock() {
        let timestamp = Instant::from_millis(get_time_ms());
        stack.iface.poll(timestamp, &mut stack.device, &mut stack.sockets);
    }
}

/// Create a TCP listen socket in smoltcp and return handle
pub fn tcp_listen(port: u16) -> Option<SmolSocketHandle> {
    if let Some(ref mut stack) = *NET_STACK.lock() {
        let tcp_rx_buffer = SocketBuffer::new(vec![0; 65535]);
        let tcp_tx_buffer = SocketBuffer::new(vec![0; 65535]);
        let mut socket = TcpSocket::new(tcp_rx_buffer, tcp_tx_buffer);
        socket.listen(port).ok()?;
        let handle = stack.sockets.add(socket);
        *TCP_LISTEN_HANDLE.lock() = Some(handle);
        Some(handle)
    } else {
        None
    }
}

/// Check if the TCP listen socket has accepted a connection
pub fn check_tcp_accept() -> bool {
    let handle = *TCP_LISTEN_HANDLE.lock();
    if let Some(handle) = handle {
        if let Some(ref mut stack) = *NET_STACK.lock() {
            let socket = stack.sockets.get::<TcpSocket>(handle);
            return socket.is_active();
        }
    }
    false
}

/// Read data from the accepted TCP connection
pub fn tcp_read(buf: &mut [u8]) -> isize {
    let handle = *TCP_LISTEN_HANDLE.lock();
    if let Some(handle) = handle {
        if let Some(ref mut stack) = *NET_STACK.lock() {
            let socket = stack.sockets.get_mut::<TcpSocket>(handle);
            if socket.can_recv() {
                match socket.recv_slice(buf) {
                    Ok(len) => return len as isize,
                    Err(_) => return -11,
                }
            }
        }
    }
    -11
}

/// Write data to the accepted TCP connection
pub fn tcp_write(data: &[u8]) -> isize {
    let handle = *TCP_LISTEN_HANDLE.lock();
    if let Some(handle) = handle {
        if let Some(ref mut stack) = *NET_STACK.lock() {
            let socket = stack.sockets.get_mut::<TcpSocket>(handle);
            if socket.can_send() {
                match socket.send_slice(data) {
                    Ok(len) => return len as isize,
                    Err(_) => return -11,
                }
            }
        }
    }
    -11
}
