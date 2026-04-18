OUTPUT_ARCH(riscv)
ENTRY(_start)

BASE_ADDRESS = 0x10000;

SECTIONS {
    . = BASE_ADDRESS;
    .text : { *(.text.entry) *(.text .text.*) }
    .rodata : { *(.rodata .rodata.*) }
    .data : { *(.data .data.*) }
    .bss : { *(.bss .bss.*) *(.sbss .sbss.*) }
    /DISCARD/ : { *(.eh_frame) *(.debug_*) }
}
