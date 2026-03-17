OUTPUT_ARCH(riscv)
ENTRY(_start)

BASE_ADDRESS = 0x80200000;

SECTIONS {
    . = BASE_ADDRESS;
    skernel = .;

    stext = .;
    .text : {
        *(.text.entry)
        strampoline = .;
        *(.text.trampoline)
        *(.text .text.*)
    }
    . = ALIGN(4K);
    etext = .;

    srodata = .;
    .rodata : {
        *(.rodata .rodata.*)
        *(.srodata .srodata.*)
    }
    . = ALIGN(4K);
    erodata = .;

    sdata = .;
    .data : {
        *(.data .data.*)
        *(.sdata .sdata.*)
    }
    . = ALIGN(4K);
    edata = .;

    .bss (NOLOAD) : {
        *(.bss.stack)
        sbss = .;
        *(.bss .bss.*)
        *(.sbss .sbss.*)
    }
    . = ALIGN(4K);
    ebss = .;

    ekernel = .;

    .initrd : {
        _initrd_start = .;
        *(.initrd)
        _initrd_end = .;
    }

    /DISCARD/ : {
        *(.eh_frame)
    }
}
