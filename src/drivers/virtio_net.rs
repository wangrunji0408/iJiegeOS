use alloc::vec::Vec;
use spin::Mutex;
use lazy_static::lazy_static;
use virtio_drivers::device::net::{VirtIONet, TxBuffer};
use virtio_drivers::transport::mmio::MmioTransport;
use super::virtio_hal::HalImpl;

const NET_QUEUE_SIZE: usize = 16;

lazy_static! {
    pub static ref VIRTIO_NET: Mutex<Option<VirtIONet<HalImpl, MmioTransport, NET_QUEUE_SIZE>>> =
        Mutex::new(None);
}

pub fn init(transport: MmioTransport) {
    match VirtIONet::<HalImpl, MmioTransport, NET_QUEUE_SIZE>::new(transport, 4096) {
        Ok(net) => {
            let mac = net.mac_address();
            println!("[NET] VirtIO-net initialized, MAC: {:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
                mac[0], mac[1], mac[2], mac[3], mac[4], mac[5]);
            *VIRTIO_NET.lock() = Some(net);
        }
        Err(e) => {
            println!("[NET] Failed to initialize VirtIO-net: {:?}", e);
        }
    }
}

pub fn can_recv() -> bool {
    if let Some(ref mut net) = *VIRTIO_NET.lock() {
        net.can_recv()
    } else {
        false
    }
}

pub fn recv() -> Option<Vec<u8>> {
    if let Some(ref mut net) = *VIRTIO_NET.lock() {
        if net.can_recv() {
            match net.receive() {
                Ok(rx_buf) => {
                    let data = rx_buf.packet().to_vec();
                    net.recycle_rx_buffer(rx_buf).ok();
                    Some(data)
                }
                Err(_) => None,
            }
        } else {
            None
        }
    } else {
        None
    }
}

pub fn send(buf: &[u8]) -> bool {
    if let Some(ref mut net) = *VIRTIO_NET.lock() {
        let tx_buf = TxBuffer::from(buf);
        match net.send(tx_buf) {
            Ok(_) => true,
            Err(_) => false,
        }
    } else {
        false
    }
}

pub fn mac_address() -> [u8; 6] {
    if let Some(ref net) = *VIRTIO_NET.lock() {
        net.mac_address()
    } else {
        [0x52, 0x54, 0x00, 0x12, 0x34, 0x56]
    }
}
