# Kernel

The kernel core is written in Rust (`kernel/rust/`), wrapped by a thin C++
runtime and linked into one ELF. Read [architecture.md](architecture.md) for
the whole picture; this file is the Rust deep-dive.

## Entry and ownership

```
kernel_entry(mb_info)          C++ (entry.cc)
   ├─ run_global_ctor_tors()
   └─ pippin_core_main(mb_info)    Rust (lib.rs) — never returns
         ├─ serial::init(0x3F8)    + writeln! bootstrap banner
         ├─ mem::multiboot_upper_memory(mb_info)
         ├─ ffi::cpp_version()      ← reverse bridge into C++
         ├─ ffi::driver_count()     ← reverse bridge into drivers
         └─ loop { cpu::halt() }
```

The Rust crate is `no_std`, dependency-free (`Cargo.toml` `[dependencies]` is
intentionally empty) and compiled with `panic=abort`. It exposes a single C
entry (`pippin_core_main`) plus whatever the C++/driver layer needs to call
back into (`kernel/rust/src/ffi.rs`). Nothing enters Rust but through
`extern "C"` — this keeps the language boundary trivial to audit.

## Modules today

| Module        | Contents |
|---------------|----------|
| `cpu.rs`      | `outb`/`inb` port I/O, `halt`, `cli`/`sti` (inline `asm!`) |
| `serial.rs`   | 16550 COM1 driver; `stdout()` -> `core::fmt::Write`; `is_ready()` |
| `mem.rs`      | layout constants, `align_up`, Multiboot v1 upper-memory read |
| `ffi.rs`      | `extern "C"` bridge: `pippin_cpp_version`, `pippin_driver_count` |
| `lib.rs`      | `pippin_core_main`, `#[panic_handler]` |

## Naming and the "Manager" convention

Subsystem modules follow the classic Macintosh names — `mem`, `proc`, `event`,
`ipc`, `file`, `drv`, `disp`, `win`, `menu`, `ctrl`, `res`. Each ships a
`*Manager` struct that owns its state and is constructed exactly once during
boot. This makes the Toolbox structure visible from the kernel's own source
tree, not just from the docs.

## Planned progression

- **Milestone 1 — memory & interrupts:**
  - physical frame allocator (bitmap over the memory map),
  - recursive page-table approach or Limine-provided maps for `kern_vm`,
  - higher-half remap using `HIGHER_HALF_BASE`,
  - GDT/TSS + IDT in `boot.S`-adjacent asm, all 256 vectors wired to a Rust
    dispatcher, PIC→APIC(timer) bring-up,
  - zone heap: a bump/slab "Zone" allocator backing `Box`/`Vec` (a
    `#[global_allocator]` on a static zone).
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