/// VirtIO 驱动
/// 支持 VirtIO-Block 和 VirtIO-Net

use virtio_drivers::{
    device::{
        blk::VirtIOBlk,
        net::VirtIONet,
    },
    transport::{
        mmio::{MmioTransport, VirtIOHeader},
        DeviceType,
        Transport,
    },
    Hal,
};
use alloc::boxed::Box;
use spin::Mutex;
use lazy_static::lazy_static;

/// VirtIO MMIO 地址（QEMU virt machine）
const VIRTIO_MMIO_BASE: usize = 0x10001000;
const VIRTIO_MMIO_SIZE: usize = 0x1000;
const VIRTIO_DEVICE_COUNT: usize = 8;

/// VirtIO HAL 实现（提供内存分配和 DMA）
pub struct VirtioHalImpl;

unsafe impl Hal for VirtioHalImpl {
    fn dma_alloc(pages: usize, direction: virtio_drivers::BufferDirection) -> (virtio_drivers::PhysAddr, core::ptr::NonNull<u8>) {
        // 分配连续物理页
        let frames: alloc::vec::Vec<_> = (0..pages)
            .map(|_| crate::mm::frame_alloc().expect("OOM: VirtIO DMA"))
            .collect();

        let ppn = frames[0].ppn;
        let pa = crate::mm::PhysAddr::from(ppn);

        // 防止 frame 被 drop（需要保持分配状态）
        // 简化：直接泄漏（DMA 内存不释放）
        core::mem::forget(frames);

        let va = pa.0;  // 恒等映射
        (pa.0, core::ptr::NonNull::new(va as *mut u8).unwrap())
    }

    unsafe fn dma_dealloc(paddr: virtio_drivers::PhysAddr, vaddr: core::ptr::NonNull<u8>, pages: usize) -> i32 {
        // 简化：不释放 DMA 内存
        0
    }

    unsafe fn mmio_phys_to_virt(paddr: virtio_drivers::PhysAddr, size: usize) -> core::ptr::NonNull<u8> {
        // 恒等映射
        core::ptr::NonNull::new(paddr as *mut u8).unwrap()
    }

    unsafe fn share(buffer: core::ptr::NonNull<[u8]>, direction: virtio_drivers::BufferDirection) -> virtio_drivers::PhysAddr {
        // 恒等映射下，虚拟地址 = 物理地址
        buffer.as_ptr() as *mut u8 as usize
    }

    unsafe fn unshare(paddr: virtio_drivers::PhysAddr, buffer: core::ptr::NonNull<[u8]>, direction: virtio_drivers::BufferDirection) {
        // 不需要做任何事
    }
}

type VirtioBlkDev = VirtIOBlk<VirtioHalImpl, MmioTransport>;
type VirtioNetDev = VirtIONet<VirtioHalImpl, MmioTransport, 16>;

lazy_static! {
    pub static ref BLK_DEVICE: Mutex<Option<VirtioBlkDev>> = Mutex::new(None);
    pub static ref NET_DEVICE: Mutex<Option<VirtioNetDev>> = Mutex::new(None);
}

pub fn probe_virtio_devices(_dtb_pa: usize) {
    // 探测每个 VirtIO MMIO 设备
    for i in 0..VIRTIO_DEVICE_COUNT {
        let base = VIRTIO_MMIO_BASE + i * VIRTIO_MMIO_SIZE;
        probe_device(base);
    }
}

fn probe_device(base: usize) {
    let header = core::ptr::NonNull::new(base as *mut VirtIOHeader).unwrap();

    // MmioTransport::new validates magic, version, and device_id
    let transport = match unsafe { MmioTransport::new(header) } {
        Ok(t) => t,
        Err(_) => return,
    };

    let device_type = transport.device_type();
    log::info!("VirtIO device at {:#x}: type={:?}", base, device_type);

    match device_type {
        DeviceType::Block => {
            match VirtIOBlk::<VirtioHalImpl, MmioTransport>::new(transport) {
                Ok(mut blk) => {
                    log::info!("VirtIO Block: capacity={} sectors", blk.capacity());
                    *BLK_DEVICE.lock() = Some(blk);
                }
                Err(e) => log::error!("VirtIO Block init failed: {:?}", e),
            }
        }
        DeviceType::Network => {
            match VirtIONet::<VirtioHalImpl, MmioTransport, 16>::new(transport, 4096) {
                Ok(mut net) => {
                    let mac = net.mac_address();
                    log::info!("VirtIO Net: MAC={:x}:{:x}:{:x}:{:x}:{:x}:{:x}",
                        mac[0], mac[1], mac[2], mac[3], mac[4], mac[5]);
                    crate::net::setup_interface(mac);
                    *NET_DEVICE.lock() = Some(net);
                }
                Err(e) => log::error!("VirtIO Net init failed: {:?}", e),
            }
        }
        _ => {}
    }
}

pub fn init_virtio() {
    // 已在 probe 中完成
}

pub fn handle_virtio_interrupt(irq: usize) {
    // VirtIO 中断处理
    // 实际上需要检查哪个设备产生了中断
    let device_idx = irq - 1;
    let base = VIRTIO_MMIO_BASE + device_idx * VIRTIO_MMIO_SIZE;

    // 读取设备类型来决定如何处理
    let header = core::ptr::NonNull::new(base as *mut VirtIOHeader).unwrap();
    let transport = match unsafe { MmioTransport::new(header) } {
        Ok(t) => t,
        Err(_) => return,
    };

    match transport.device_type() {
        DeviceType::Block => {
            // 块设备中断（完成一次 I/O）
        }
        DeviceType::Network => {
            // 网络设备中断（收到数据包）
            if let Some(ref mut net) = NET_DEVICE.lock().as_mut() {
                while let Ok(rx_buf) = net.receive() {
                    let pkt = rx_buf.packet();
                    if !pkt.is_empty() {
                        // 将数据包传递给 smoltcp
                        crate::net::poll();
                    }
                }
            }
        }
        _ => {}
    }
}

/// 块设备读写接口
pub fn read_block(block_id: usize, buf: &mut [u8]) -> Result<(), &'static str> {
    let mut dev = BLK_DEVICE.lock();
    if let Some(ref mut blk) = dev.as_mut() {
        blk.read_blocks(block_id, buf).map_err(|_| "block read failed")
    } else {
        Err("no block device")
    }
}

pub fn write_block(block_id: usize, buf: &[u8]) -> Result<(), &'static str> {
    let mut dev = BLK_DEVICE.lock();
    if let Some(ref mut blk) = dev.as_mut() {
        blk.write_blocks(block_id, buf).map_err(|_| "block write failed")
    } else {
        Err("no block device")
    }
}
