# Boot

Pippin has two x86-64 boot images. Both reach the same `no_std` Rust core and
use the serial console for bring-up. The Limine ISO is the M1 boot path; the
Multiboot image remains useful for a fast `qemu-system-x86_64 -kernel` loop.

## Limine protocol (`make iso`)

`build/kernel-limine.elf` is a 64-bit higher-half ELF linked at
`0xffffffff80000000`. Limine enters `limine_start` in long mode. The C++ entry
checks the Limine base revision, reads its memory-map response, runs global
constructors, and calls `pippin_core_main_limine`. Rust releases only memory
that Limine marks usable, so the kernel image and bootloader structures stay
reserved. The kernel initially uses Limine's page tables and identity map.

The ISO includes BIOS and UEFI Limine binaries. `make iso` requires Limine and
`xorriso`; see [build.md](build.md). Limine base revision 0 is requested for
the initial identity map used by the current frame allocator and APIC driver.
Moving to a newer base revision will require an HHDM-aware physical access
layer.

## Multiboot development image (`make run`)

`build/kernel.elf` begins with the Multiboot v1 header in `boot.S`. The header
includes physical load, BSS, and entry addresses, allowing QEMU to byte-copy
the 64-bit ELF image. `_start` saves the boot information pointer, clears boot
scratch, enables PAE and long mode, and maps the first 1 GiB both identity and
at `0xffff800000000000`. It then jumps to the higher-half entry, calls the C++
`kernel_entry`, and hands control to `pippin_core_main` in Rust.

Rust replaces the temporary 2 MiB mapping with a 4 KiB page-table hierarchy.
The frame allocator reserves the full kernel image before handing out memory.

## Common core bring-up

Both paths initialize COM1 serial, the frame bitmap, and the zone heap. The
kernel installs its own GDT and TSS, with an interrupt stack for double
faults, then loads all 256 IDT vectors. The PIT supplies initial ticks while
the local APIC timer is calibrated. If the APIC starts, it becomes the 1000 Hz
timer and legacy IRQ0 is masked; otherwise the PIT remains the timer. An `int3`
and a 250 ms sleep check interrupt dispatch before the idle loop.

Pippin keeps floating-point work out of the kernel core. The Multiboot path
enables SSE in CR4 before entering Rust; Limine provides a 64-bit handoff with
SSE available. M2 saves each task's FP/XMM state on interrupt switches.
