/* Limine loads a 64-bit ELF directly into the higher half. */
ENTRY(limine_start)

SECTIONS
{
  . = 0xffffffff80000000;
  .text : ALIGN(4K) { *(.text .text.* .ltext .ltext.*) }
  .rodata : ALIGN(4K) { *(.rodata .rodata.* .lrodata .lrodata.*) *(.gcc_except_table*) }
  .data : ALIGN(4K) {
    *(.data .data.* .ldata .ldata.*)
    *(.got .got.*)
    . = ALIGN(8);
    KEEP(*(.limine_requests_start))
    KEEP(*(.limine_requests))
    KEEP(*(.limine_requests_end))
  }
  .init_array : ALIGN(8) {
    __init_array_start = .;
    KEEP(*(.init_array*))
    __init_array_end = .;
  }
  .fini_array : ALIGN(8) {
    __fini_array_start = .;
    KEEP(*(.fini_array*))
    __fini_array_end = .;
  }
  .bss : ALIGN(4K) {
    __bss_start = .;
    *(.bss .bss.* .lbss .lbss.*)
    *(COMMON)
    . = ALIGN(4K);
    __bss_end = .;
  }
  /DISCARD/ : { *(.comment) *(.eh_frame*) *(.note .note.*) }
}
