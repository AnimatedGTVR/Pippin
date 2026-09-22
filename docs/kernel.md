# Kernel

The kernel core is written in Rust (`kernel/rust/`), wrapped by a thin C++
runtime and linked into both boot ELFs. Read [architecture.md](architecture.md) for
the whole picture; this file is the Rust deep-dive.

## Entry and ownership

```
kernel_entry(mb_info) or limine_start()     C++
   ├─ run global constructors
   └─ pippin_core_main(_limine)             Rust (lib.rs) — never returns
         ├─ serial::init(0x3F8)    + writeln! bootstrap banner
         ├─ memory map → frame bitmap → page tables / heap
         ├─ ffi::cpp_version()      ← reverse bridge into C++
         ├─ ffi::driver_count()     ← reverse bridge into drivers
         ├─ GDT/TSS → IDT → PIT → APIC timer
         └─ loop { cpu::halt() }
```

The Rust crate is `no_std`, dependency-free (`Cargo.toml` `[dependencies]` is
intentionally empty) and compiled with `panic=abort`. It exposes a single C
entry for each boot path plus whatever the C++/driver layer needs to call
back into (`kernel/rust/src/ffi.rs`). Nothing enters Rust but through
`extern "C"` — this keeps the language boundary trivial to audit.

## Modules today

| Module        | Contents |
|---------------|----------|
| `cpu.rs`      | `outb`/`inb` port I/O, `halt`, `cli`/`sti` (inline `asm!`) |
| `serial.rs`   | 16550 COM1 driver; `stdout()` -> `core::fmt::Write`; `is_ready()` |
| `mem.rs`      | Multiboot/Limine memory maps and physical frame bitmap |
| `mm.rs`       | 4 KiB page tables for the Multiboot path and MMIO mapping |
| `heap.rs`     | zone allocator backing `Box` and `Vec` |
| `gdt.rs`      | GDT, TSS, and double-fault stack |
| `idt.rs`/`interrupts.rs` | 256-vector IDT, PIC/PIT and interrupt dispatch |
| `apic.rs`     | calibrated local APIC timer |
| `ffi.rs`      | `extern "C"` bridge: `pippin_cpp_version`, `pippin_driver_count` |
| `lib.rs`      | `pippin_core_main`, `#[panic_handler]` |

## Naming and the "Manager" convention

Subsystem modules follow the classic Macintosh names — `mem`, `proc`, `event`,
`ipc`, `file`, `drv`, `disp`, `win`, `menu`, `ctrl`, `res`. Each ships a
`*Manager` struct that owns its state and is constructed exactly once during
boot. This makes the Toolbox structure visible from the kernel's own source
tree, not just from the docs.

## Milestone progression

- **Milestone 1 — memory & interrupts (complete):**
  - physical frame bitmap over the bootloader memory map,
  - higher-half 4 KiB tables on Multiboot, Limine page tables on the ISO path,
  - GDT/TSS, 256-vector IDT, Rust dispatcher, and PIT→APIC timer handoff,
  - zone allocator backing `Box`/`Vec` through `#[global_allocator]`.
- **Milestone 2 — processes & syscalls:**
  - `syscall`/`sysret` trampolines (asm), numbered table with names from
    `pippin::kabi::Syscall` (`kernel/cpp/include/pippin/kernel.hh`),
  - scheduler: preemptive, per-task FP/XMM save (fxsave/fxrstor), quantum
    timers,
  - IPC: message ports carrying typed `Event`s into the event loop.
- **Milestone 3 — file & drivers:** VFS + FAT (then ISO9660), the Driver
  Manager consuming C++ `Driver`s.

## Interrupt discipline

Until the IDT exists, interrupts are off and the world is single-threaded.
`cpu::cli`/`cpu::sti` are the only toggles; the serial writer locks nothing and
is callable from the panic handler. When preemption arrives, every shared
structure grows a proper spinlock — the panic handler must stay interrupt-safe
(it is: serial writes are byte-blocking and non-recursive).

## FP/XMM policy

- Code in the **Rust core and C++ kernel subsystems must not use float math**.
  This keeps kernel-internal paths free of FP state save/restore and of #NM/#
  XM surprises.
- SSE is nonetheless initialized at boot (CR4.OSFXSR) because compilers emit
  integer SIMD (see [boot.md](boot.md) for the #UD story).
- Once tasks exist, the scheduler saves/restores the full FPU/XMM context
  (`fxsave64`/`fxrstor64`) per task, so strictly-float drivers are possible
  later without poisoning the core.

## Adding something new

1. Pick the right layer: CPU/asm → `kernel/asm`, core logic → `kernel/rust`,
   subsystem/driver → `kernel/cpp` or `drivers/cpp`, ABI shim → `kernel/c`.
2. Cross a language boundary only through a new `extern "C"` pair (declare in
   `ffi.rs` / a `kernel/*/include` header), and add the source file to the
   matching `CMakeLists.txt` (for Rust, also to the `DEPENDS` list of the
   `rust-kernel` custom command in the root `CMakeLists.txt`).
3. Rebuild `make`, boot `make run`, watch the serial console.
