.PHONY: all user kernel run run-nginx clean

QEMU = qemu-system-riscv64
QEMU_OPTS = -machine virt -nographic -bios default \
            -netdev user,id=net0,hostfwd=tcp::5555-:80 \
            -device virtio-net-device,netdev=net0

KERNEL = target/riscv64gc-unknown-none-elf/release/kernel

all: user kernel

user:
	cd user/hello && cargo build --release
	cd user/httpd && cargo build --release

kernel: user
	cargo build --release -p kernel

run: all
	$(QEMU) $(QEMU_OPTS) -kernel $(KERNEL)

# curl against the running kernel's HTTP server
curl:
	curl -v http://localhost:5555/

clean:
	cargo clean
	rm -rf user/hello/target user/httpd/target
