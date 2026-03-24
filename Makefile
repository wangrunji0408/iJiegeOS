TARGET := riscv64gc-unknown-none-elf
MODE := release
KERNEL_ELF := target/$(TARGET)/$(MODE)/jiegeos
KERNEL_BIN := $(KERNEL_ELF).bin
OBJCOPY := rust-objcopy --binary-architecture=riscv64

QEMU := qemu-system-riscv64
QEMU_ARGS := -machine virt \
	-nographic \
	-bios default \
	-m 256M \
	-kernel $(KERNEL_BIN) \
	-netdev user,id=net0,hostfwd=tcp::8080-:80 \
	-device virtio-net-device,netdev=net0

ifeq ($(MODE), release)
	CARGO_FLAGS := --release
endif

.PHONY: build kernel clean run debug

build: kernel

kernel:
	cargo build $(CARGO_FLAGS)
	$(OBJCOPY) $(KERNEL_ELF) --strip-all -O binary $(KERNEL_BIN)

clean:
	cargo clean

run: kernel
	$(QEMU) $(QEMU_ARGS)

debug: kernel
	$(QEMU) $(QEMU_ARGS) -s -S

gdb:
	riscv64-elf-gdb -ex 'file $(KERNEL_ELF)' -ex 'target remote localhost:1234'
