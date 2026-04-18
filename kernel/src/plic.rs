//! Platform-Level Interrupt Controller on qemu-virt. Stubbed for now.
pub fn init() {}
pub fn handle_external() {
    crate::net::virtio_irq();
}
