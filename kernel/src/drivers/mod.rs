mod virtio;

pub use virtio::init_virtio;

use spin::Mutex;

pub fn init(dtb_pa: usize) {
    // 初始化 PLIC（平台级中断控制器）
    // 先跳过 PLIC 初始化进行测试
    // init_plic();

    // 探测并初始化 VirtIO 设备
    virtio::probe_virtio_devices(dtb_pa);

    log::info!("drivers: all devices initialized");
}

fn init_plic() {
    // QEMU virt machine PLIC 地址: 0x0c000000
    // 配置 PLIC 让 hart 0 接收中断
    const PLIC_BASE: usize = 0x0c000000;

    // 使能所有中断源（简化）
    // 实际上需要为每个中断源设置优先级和使能位
    let plic = unsafe { &mut *(PLIC_BASE as *mut u32) };

    // 设置 hart 0 的 M/S 模式阈值为 0（接受所有中断）
    // PLIC_SENABLE[hart] at 0x0c002000 + hart * 0x80
    // PLIC_SPRIORITY[hart] at 0x0c201000 + hart * 0x1000
    // PLIC_SCLAIM[hart] at 0x0c200004 + hart * 0x1000

    unsafe {
        // 设置 VirtIO 中断优先级（中断 1-8）
        for i in 1..9usize {
            let priority_addr = (PLIC_BASE + i * 4) as *mut u32;
            *priority_addr = 1;
        }

        // 使能 hart 0 S 模式的 VirtIO 中断（1-8）
        let senable_addr = (PLIC_BASE + 0x2080) as *mut u32;
        *senable_addr = 0x1fe;  // 位 1-8

        // 设置阈值为 0
        let threshold_addr = (PLIC_BASE + 0x201000) as *mut u32;
        *threshold_addr = 0;
    }

    log::info!("PLIC initialized");
}

/// 处理外部中断
pub fn handle_external_interrupt() {
    const PLIC_BASE: usize = 0x0c000000;
    const PLIC_CLAIM: usize = PLIC_BASE + 0x201004;

    // 读取 PLIC claim 寄存器获取中断源
    let irq = unsafe { *(PLIC_CLAIM as *const u32) };

    if irq == 0 { return; }

    // 处理中断
    match irq {
        1..=8 => {
            // VirtIO 设备中断
            virtio::handle_virtio_interrupt(irq as usize);
        }
        _ => {
            log::warn!("Unknown IRQ: {}", irq);
        }
    }

    // 通知 PLIC 中断处理完成
    unsafe { *(PLIC_CLAIM as *mut u32) = irq; }
}
