mod socket;

pub use socket::*;

use alloc::vec;
use alloc::vec::Vec;
use spin::Mutex;
use lazy_static::lazy_static;
use smoltcp::iface::{Config, Interface, SocketSet};
use smoltcp::wire::{EthernetAddress, IpCidr, Ipv4Address, IpAddress};
use smoltcp::phy::{Device, DeviceCapabilities, Medium, RxToken, TxToken};
use smoltcp::time::Instant;

lazy_static! {
    pub static ref NET_STACK: Mutex<Option<NetStack>> = Mutex::new(None);
}

pub struct NetStack {
    pub iface: Interface,
    pub sockets: SocketSet<'static>,
    pub device: VirtioNetDevice,
}

pub struct VirtioNetDevice {
    rx_queue: Vec<Vec<u8>>,
    tx_queue: Vec<Vec<u8>>,
    mac: [u8; 6],
}

impl VirtioNetDevice {
    pub fn new(mac: [u8; 6]) -> Self {
        Self {
            rx_queue: Vec::new(),
            tx_queue: Vec::new(),
            mac,
        }
    }

    pub fn receive(&mut self, data: Vec<u8>) {
        self.rx_queue.push(data);
    }
}

struct VirtioRxToken(Vec<u8>);
struct VirtioTxToken<'a>(&'a mut VirtioNetDevice);

impl RxToken for VirtioRxToken {
    fn consume<R, F>(self, f: F) -> R
    where
        F: FnOnce(&[u8]) -> R,
    {
        f(&self.0)
    }
}

impl<'a> TxToken for VirtioTxToken<'a> {
    fn consume<R, F>(self, len: usize, f: F) -> R
    where
        F: FnOnce(&mut [u8]) -> R,
    {
        let mut buf = vec![0u8; len];
        let result = f(&mut buf);
        self.0.tx_queue.push(buf);
        result
    }
}

impl Device for VirtioNetDevice {
    type RxToken<'a> = VirtioRxToken;
    type TxToken<'a> = VirtioTxToken<'a>;

    fn receive(&mut self, _timestamp: Instant) -> Option<(Self::RxToken<'_>, Self::TxToken<'_>)> {
        if self.rx_queue.is_empty() {
            None
        } else {
            let data = self.rx_queue.remove(0);
            Some((VirtioRxToken(data), VirtioTxToken(self)))
        }
    }

    fn transmit(&mut self, _timestamp: Instant) -> Option<Self::TxToken<'_>> {
        Some(VirtioTxToken(self))
    }

    fn capabilities(&self) -> DeviceCapabilities {
        let mut caps = DeviceCapabilities::default();
        caps.medium = Medium::Ethernet;
        caps.max_transmission_unit = 1514;
        caps
    }
}

pub fn init() {
    let mac = [0x52, 0x54, 0x00, 0x12, 0x34, 0x56];
    let device = VirtioNetDevice::new(mac);

    let config = Config::new(EthernetAddress(mac).into());
    let mut iface = Interface::new(config, &mut &mut &device, Instant::ZERO);

    iface.update_ip_addrs(|addrs| {
        addrs.push(IpCidr::new(IpAddress::v4(10, 0, 2, 15), 24)).unwrap();
    });

    // Set default gateway
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
