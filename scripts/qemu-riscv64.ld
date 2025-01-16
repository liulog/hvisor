ENTRY(arch_entry)
BASE_ADDRESS = 0x80200000;


SECTIONS
{
    . = BASE_ADDRESS;
    skernel = .;

    stext = .;
    .text : {
        *(.text.entry)
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
    .bss : {
        *(.bss.stack)
        sbss = .;
        *(.bss .bss.*)
        *(.sbss .sbss.*)
    }

    . = ALIGN(4K);
    ebss = .;
    ekernel = .;

    /DISCARD/ : {
        *(.eh_frame)
    }
    . = ALIGN(4K);
	__core_end = .;

    /* 注意：这后面的部分区域被使用了 */
    /* 8.5M， 目前大约到 0x80A80000 */

    . = 0x80E00000;         /* 留给设备树 */
    .dtb1 : {
        KEEP(*(.dtb1))
    }

    . = 0x81000000;     /* linux 本身也会空出前面的 2MB 空间 */
    .img1 : {
        KEEP(*(.img1))
    }
}