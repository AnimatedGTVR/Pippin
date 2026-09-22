/* Pippin kernel linker script.
 *
 * Layout: the Multiboot loader drops the image at physical 1 MiB
 * (KERNEL_LOAD_BASE). boot.S runs there in a LOW boot segment, builds an
 * identity map of the low 1 GiB plus a higher-half alias, and far-jumps
 * into the high kernel. Everything after ".boot" is linked at
 * KERNEL_VMA_BASE + physical-offset, i.e. a true higher-half kernel.
 *
 * Physical placement: each high section's LMA is forced to
 *   vma - KERNEL_VMA_BASE
 * so the on-disk image stays physically contiguous with the boot segment
 * (the condition both the ELF segments and QEMU's Multiboot byte-copy rely
 * on). The Multiboot header's address fields carry these physical values.
 */

ENTRY(_start)

KERNEL_LOAD_BASE = 0x100000;
KERNEL_VMA_BASE  = 0xFFFF800000000000;

SECTIONS
{
  /* ---- Low boot segment (runs before the higher-half hop) ------------- */
  . = KERNEL_LOAD_BASE;

  . = ALIGN(4K);
  .multiboot :
  {
    KEEP(*(.multiboot))
  }

  . = ALIGN(16);
  .boot.text :
  {
    *(.boot.text*)
  }

  . = ALIGN(8);
  .boot.rodata :
  {
    *(.boot.rodata*)
  }

  . = ALIGN(16);
  .boot.data :
  {
    *(.boot.data*)
  }
  __boot_data_end = .;

  /* Early scratch (page tables, stack). File-backed zeros (NOT NOLOAD):
   * QEMU's HAS_ADDR path maps the file linearly onto physical memory
   * starting at the header, so a hole here would misplace every later
   * section. boot.S re-zeroes this window anyway. */
  . = ALIGN(16);
  .boot.bss :
  {
    __boot_bss_start = .;
    *(.boot.bss*)
    __boot_bss_end = .;
  }
  __boot_end = .;

  /* ---- Higher-half kernel -------------------------------------------- */
  . += KERNEL_VMA_BASE;

  . = ALIGN(4K);
  .text : AT(ADDR(.text) - KERNEL_VMA_BASE)
  {
    *(.text .text.*)
  }

  . = ALIGN(4K);
  .rodata : AT(ADDR(.rodata) - KERNEL_VMA_BASE)
  {
    *(.rodata .rodata.*)
  }

  . = ALIGN(4K);
  .data : AT(ADDR(.data) - KERNEL_VMA_BASE)
  {
    *(.data .data.*)
  }

  /* C++ global constructors, called by kernel/cpp entry.                  */
  . = ALIGN(8);
  .init_array : AT(ADDR(.init_array) - KERNEL_VMA_BASE)
  {
    __init_array_start = .;
    KEEP(*(.init_array*))
    __init_array_end = .;
  }
  . = ALIGN(8);
  .fini_array : AT(ADDR(.fini_array) - KERNEL_VMA_BASE)
  {
    __fini_array_start = .;
    KEEP(*(.fini_array*))
    __fini_array_end = .;
  }

  /* End of file-backed data (VMA) + its physical alias for the Multiboot
   * header's load_end_addr. */
  __data_end = .;
  __data_end_phys = __data_end - KERNEL_VMA_BASE;

  /* Zeroed memory; __bss_start..__bss_end is the high kernel BSS only.
   * The low .boot.bss is cleared by boot.S itself. */
  . = ALIGN(4K);
  .bss : AT(ADDR(.bss) - KERNEL_VMA_BASE)
  {
    __bss_start = .;
    *(.bss .bss.*)
    *(COMMON)
    . = ALIGN(4K);
    __bss_end = .;
  }
  __bss_end_phys = __bss_end - KERNEL_VMA_BASE;

  /DISCARD/ :
  {
    *(.comment)
    *(.eh_frame*)
    *(.note .note.*)
    *(.rela.*)
  }
}
