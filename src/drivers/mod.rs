mod virtio_hal;
pub mod virtio_net;

use virtio_drivers::transport::mmio::{MmioTransport, VirtIOHeader};
use virtio_drivers::transport::{DeviceType, Transport};

pub fn init(_dtb: usize) {
    // Probe VirtIO MMIO devices at known QEMU virt addresses
    for i in 0..8 {
        let addr = 0x10001000 + i * 0x1000;
        probe_virtio_mmio(addr);
    }
}

fn probe_virtio_mmio(addr: usize) {
    let header = unsafe { &mut *(addr as *mut VirtIOHeader) };

    if let Ok(transport) = unsafe { MmioTransport::new(core::ptr::NonNull::from(header)) } {
        let device_type = transport.device_type();
        println!("[VIRTIO] Found {:?} at {:#x}", device_type, addr);

        match device_type {
            DeviceType::Network => {
                virtio_net::init(transport);
            }
            _ => {}
        }
    }
}
