//! VirtIO-net driver + smoltcp glue.
use crate::virtio_hal::KernelHal;
use alloc::boxed::Box;
use alloc::vec;
use alloc::vec::Vec;
use alloc::sync::Arc;
use core::cell::RefCell;
use smoltcp::iface::{Config, Interface, SocketHandle, SocketSet};
use smoltcp::phy::{Device, DeviceCapabilities, Medium, RxToken, TxToken};
use smoltcp::socket::tcp;
use smoltcp::time::Instant;
use smoltcp::wire::{EthernetAddress, HardwareAddress, IpAddress, IpCidr, Ipv4Address};
use spin::Mutex;
use virtio_drivers::device::net::{TxBuffer, VirtIONet};
use virtio_drivers::transport::mmio::{MmioTransport, VirtIOHeader};
use virtio_drivers::transport::{DeviceType, Transport};

pub const VIRTIO_MMIO_BASE: usize = 0x1000_1000;
pub const VIRTIO_MMIO_STRIDE: usize = 0x1000;
pub const VIRTIO_MMIO_SLOTS: usize = 8;

const NET_QUEUE_SIZE: usize = 16;
const NET_BUF_LEN: usize = 2048;

type Nic = VirtIONet<KernelHal, MmioTransport<'static>, NET_QUEUE_SIZE>;

pub struct VirtioNetDevice {
    pub nic: Arc<Mutex<Nic>>,
}

impl VirtioNetDevice {
    pub fn probe() -> Option<Self> {
        for i in 0..VIRTIO_MMIO_SLOTS {
            let base = VIRTIO_MMIO_BASE + i * VIRTIO_MMIO_STRIDE;
            let hdr = core::ptr::NonNull::new(base as *mut VirtIOHeader)?;
            let transport = match unsafe { MmioTransport::new(hdr, VIRTIO_MMIO_STRIDE) } {
                Ok(t) => t,
                Err(_) => continue,
            };
            if transport.device_type() == DeviceType::Network {
                crate::println!("[virtio-net] found at {:#x}", base);
                let nic = Nic::new(transport, NET_BUF_LEN).ok()?;
                return Some(Self { nic: Arc::new(Mutex::new(nic)) });
            }
        }
        None
    }

    pub fn mac(&self) -> [u8; 6] { self.nic.lock().mac_address() }
}

// --- smoltcp adapter ---------------------------------------------------

pub struct SmolDev<'a> {
    pub inner: &'a VirtioNetDevice,
}

pub struct RxTok(pub Vec<u8>);
pub struct TxTok<'a> {
    pub dev: &'a VirtioNetDevice,
}

impl<'a> Device for SmolDev<'a> {
    type RxToken<'b> = RxTok where Self: 'b;
    type TxToken<'b> = TxTok<'b> where Self: 'b;

    fn receive(&mut self, _ts: Instant) -> Option<(Self::RxToken<'_>, Self::TxToken<'_>)> {
        let mut nic = self.inner.nic.lock();
        if !nic.can_recv() { return None; }
        match nic.receive() {
            Ok(rx_buf) => {
                let data = rx_buf.packet().to_vec();
                let _ = nic.recycle_rx_buffer(rx_buf);
                Some((RxTok(data), TxTok { dev: self.inner }))
            }
            Err(_) => None,
        }
    }

    fn transmit(&mut self, _ts: Instant) -> Option<Self::TxToken<'_>> {
        let nic = self.inner.nic.lock();
        if nic.can_send() { drop(nic); Some(TxTok { dev: self.inner }) } else { None }
    }

    fn capabilities(&self) -> DeviceCapabilities {
        let mut caps = DeviceCapabilities::default();
        caps.medium = Medium::Ethernet;
        caps.max_transmission_unit = 1500;
        caps.max_burst_size = Some(1);
        caps
    }
}

impl RxToken for RxTok {
    fn consume<R, F>(self, f: F) -> R where F: FnOnce(&[u8]) -> R {
        f(&self.0)
    }
}

impl<'a> TxToken for TxTok<'a> {
    fn consume<R, F>(self, len: usize, f: F) -> R where F: FnOnce(&mut [u8]) -> R {
        let mut buf = vec![0u8; len];
        let r = f(&mut buf);
        let mut nic = self.dev.nic.lock();
        let _ = nic.send(TxBuffer::from(buf.as_slice()));
        r
    }
}

// --- Network state -----------------------------------------------------

pub struct Net {
    pub dev: VirtioNetDevice,
    pub iface: Interface,
    pub sockets: SocketSet<'static>,
}

static NET: Mutex<Option<Net>> = Mutex::new(None);

pub fn init() {
    let dev = match VirtioNetDevice::probe() {
        Some(d) => d,
        None => { crate::println!("[virtio-net] not found"); return; }
    };
    let mac = dev.mac();
    crate::println!("[virtio-net] mac = {:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
        mac[0], mac[1], mac[2], mac[3], mac[4], mac[5]);
    let mut smol = SmolDev { inner: &dev };
    let cfg = Config::new(HardwareAddress::Ethernet(EthernetAddress(mac)));
    let mut iface = Interface::new(cfg, &mut smol, Instant::from_millis(now_ms() as i64));
    iface.update_ip_addrs(|addrs| {
        let _ = addrs.push(IpCidr::new(IpAddress::v4(10, 0, 2, 15), 24));
    });
    iface.routes_mut().add_default_ipv4_route(Ipv4Address::new(10, 0, 2, 2)).ok();
    let sockets = SocketSet::new(Vec::new());
    *NET.lock() = Some(Net { dev, iface, sockets });
    crate::println!("[virtio-net] ip = 10.0.2.15/24, gw = 10.0.2.2");
}

fn now_ms() -> u64 { crate::timer::now_ms() }

pub fn poll() {
    if let Some(net) = NET.lock().as_mut() {
        let ts = Instant::from_millis(now_ms() as i64);
        let mut smol = SmolDev { inner: &net.dev };
        net.iface.poll(ts, &mut smol, &mut net.sockets);
    }
}

pub fn virtio_irq() { poll(); }

// --- Socket abstraction for the Linux syscall layer --------------------

pub struct Socket {
    pub handle: SocketHandle,
}

pub fn tcp_open() -> Option<Socket> {
    let rx_buf = tcp::SocketBuffer::new(vec![0; 8192]);
    let tx_buf = tcp::SocketBuffer::new(vec![0; 8192]);
    let sock = tcp::Socket::new(rx_buf, tx_buf);
    let mut net = NET.lock();
    let net = net.as_mut()?;
    let handle = net.sockets.add(sock);
    Some(Socket { handle })
}

pub fn tcp_bind_listen(sock: &Socket, port: u16) -> bool {
    let mut net = NET.lock();
    let Some(net) = net.as_mut() else { return false; };
    let s = net.sockets.get_mut::<tcp::Socket>(sock.handle);
    s.listen(port).is_ok()
}

pub fn tcp_is_active(sock: &Socket) -> bool {
    let mut net = NET.lock();
    let Some(net) = net.as_mut() else { return false; };
    let s = net.sockets.get_mut::<tcp::Socket>(sock.handle);
    s.is_active()
}

pub fn tcp_send(sock: &Socket, data: &[u8]) -> isize {
    let mut net = NET.lock();
    let Some(net) = net.as_mut() else { return -1; };
    let s = net.sockets.get_mut::<tcp::Socket>(sock.handle);
    match s.send_slice(data) {
        Ok(n) => n as isize,
        Err(_) => -1,
    }
}

pub fn tcp_recv(sock: &Socket, data: &mut [u8]) -> isize {
    let mut net = NET.lock();
    let Some(net) = net.as_mut() else { return -1; };
    let s = net.sockets.get_mut::<tcp::Socket>(sock.handle);
    match s.recv_slice(data) {
        Ok(n) => n as isize,
        Err(_) => -1,
    }
}
