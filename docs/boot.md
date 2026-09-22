# Boot

The ASM boot path lives in `kernel/asm/boot.S`. It is the *only* assembly in
the ELF that runs before C++ and Rust; after `kernel_entry` returns, control
never goes back to it except for the idle/panic halt loop.

## Protocol: Multiboot v1, with explicit addresses

`boot.S` carries a Multiboot v1 header with the address fields
(`MULTIBOOT_HEADER_HAS_ADDR`, bit 16):

| Field          | Value            | Meaning                             |
|----------------|------------------|-------------------------------------|
| magic          | `0x1BADB002`     | Multiboot v1 id                     |
| flags          | `0x10003`        | align 4K | memory info | address fields |
| checksum       | `-(magic+flags)` |                                    |
| header_addr    | `0x100000`       | where the header itself is loaded   |
| load_addr      | `0x100000`       | start of the image in RAM           |
| load_end_addr  | `__data_end`     | end of file-backed data             |
| bss_end_addr   | `__bss_end`      | end of BSS (loader zeroes it)       |
| entry_addr     | `_start`         | physical entry point                |

Setting the ADDR fields matters: **QEMU (≤ 8.x) refuses to accept a 64-bit
ELF kernel through the Multiboot path.** With the address fields set, the
bootloader byte-copies the image to `load_addr` and never inspects the ELF
format, so a 64-bit kernel boots fine. This also shrinks boot dependencies —
the header is self-describing. GRUB handles this header identically.

```
_start (README: boot.S)
  │  save EAX (magic), EBX (mb_info)
  │  zero BSS (page tables + stack + statics)
  │  verify magic == 0x2BADB002
  │  CPUID present?  long mode supported?
  ├─ build boot_pd0: 512 × 2 MiB identity entries (low 1 GiB)
  ├─ PML4[0] → PDPT[0] → boot_pd0
  ├─ CR4 = PAE | OSFXSR | OSXMMEXCPT
  ├─ EFER.LME = 1
  ├─ CR0 = PG | WP
  ├─ LGDT (64-bit selectors) → far jump to .Lkm64
  ├─ set DS/ES/FS/GS/SS = 0x10
  ├─ RDI = mb_info ; RSP adjusted to 16
  └─ call kernel_entry          (C++, kernel/cpp/src/entry.cc)
        ├─ run __init_array (C++ global constructors)
        └─ call pippin_core_main(mb_info)   (Rust, never returns)
              └─ idle loop: hlt
```

## Why the entry chain is C++ —> Rust

The assembly trampoline, C++ runtime and Rust core are three tight layers:

1. `boot.S` — the minimum to reach long mode. Assembly owns the CPU-state
   transitions and nothing the higher layers need to touch daily.
2. `kernel_entry` — C++ because C++ global constructors (`__init_array`) are
   part of the language runtime and must run before Rust touches any
   C++-constructed object. This is also the seam where a future
   `kernel/cpp/src/runtime.cc` can install the IDT, task state and so on
   before trusting Rust.
3. `pippin_core_main` — Rust, the actual kernel.

## Early boot invariants

- Interrupts are disabled from `cli` in `_start` until the kernel installs an
  IDT + PIC/APIC and enables them (Milestone 2).
- The first 1 GiB is identity-mapped. The higher-half remap (Milestone 1)
  replaces `boot_pd0` and re-creates the kernel mapping at
  `0xFFFF800000000000+` (see `HIGHER_HALF_BASE` in `kernel/rust/src/mem.rs`).
- SSE is *enabled* (CR4.OSFXSR): rustc/LLVM freely emits integer SIMD even in
  `no_std` code (e.g. `movd xmm0` in `core::fmt::num`), so leaving it off
  causes #UD triple faults. This is safe because all FP/XMM state is saved per
  task by the Milestone-2 scheduler.

## Serial console

`kernel/rust/src/serial.rs` drives COM1 (0x3F8, 38400 8N1) as the debug
console: `serial::stdout()` returns a `core::fmt::Write` so `writeln!` works
from anywhere in the Rust core. A panic handler mirrors the message there.

## Migration to Limine (Milestone 1)

Today we boot from the hand-rolled Multiboot stub. At Milestone 1 we switch to
the Limine boot protocol for real physical memory maps, framebuffer info and
EFI support. The config is already staged at `boot/limine.conf`, and
`scripts/make-iso.sh` knows how to produce a bootable image (`limine` +
`xorriso` required). The migration keeps `boot.S` as the 64-bit entry code and
simply changes how the kernel got its info structure — the Rust `mem.rs`
helpers swap from "read QEMU's Multiboot v1 info" to "read Limine's response
struct".