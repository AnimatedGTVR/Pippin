# Build

One build, one artifact: `build/kernel.elf` — a Multiboot kernel ELF mixing
Assembly, Rust, C++, C and driver C++ sources. Everything is driven by CMake
with a convenience `Makefile` on top.

## Toolchain

This mandatory set, in the form installed on the current machine:

| Tool            | Version (tested)     | Use                             |
|-----------------|----------------------|---------------------------------|
| `cmake`         | 3.28                 | build orchestration             |
| `gcc`/`g++`     | 13.3                 | C, C++, and preprocessing `.S`  |
| `rustc`/`cargo` | 1.95 (stable)        | Rust kernel core (`no_std`)     |
| GNU `as`/`ld`   | binutils 2.42        | in-built assembler, final link  |
| `make`          | 4.3                  | top-level targets               |
| `qemu-system-x86_64` | 8.2 (optional)  | boot the OS without real HW     |

`gcc` is used both as the C/C++ compiler and as the assembler driver
(`CMAKE_ASM_COMPILER` is pinned to it in the root `CMakeLists.txt`), so `.S`
files get preprocessed for free — no separate `nasm` needed. `clang` is not
required. Networking is not required to build (see the Rust note below).

## Building

```sh
make            # configure + compile everything -> build/kernel.elf
make run        # boot in QEMU, serial console to your terminal
make run-gdb    # QEMU paused, GDB stub on :1234
make iso        # build a Limine ISO (needs limine + xorriso; Milestone 1+)
make clean      # rm build/
make distclean  # also wipe the cargo target dir
```

or raw CMake:

```sh
cmake -S . -B build -G "Unix Makefiles"
cmake --build build --target kernel.elf -j
qemu-system-x86_64 -machine q35 -m 256M -display none -serial stdio \
    -kernel build/kernel.elf
```

## How the languages meet

1. **Assembly** (`kernel/asm/boot.S`) — Multiboot header + long-mode bring-up.
   Compiled by the C compiler as an `ASM` source.
2. **Rust** (`kernel/rust`) — `cargo build --release` produces
   `libpippin_kernel.a` (a `staticlib`). The root `CMakeLists.txt` wraps that
   in a custom command/target (`rust-kernel`) and feeds the archive to the
   final link.
3. **C++ + C + drivers** — ordinary CMake `STATIC` libraries.
4. **Final link** (`kernel/CMakeLists.txt`) — one `ld` pass with
   - `-nostdlib`, `-no-pie` (plain `ET_EXEC`, required for Multiboot),
   - the linker script `kernel/linker.ld`,
   - all archives wrapped in `-Wl,--start-group/--end-group` because they
     reference each other in both directions (Rust ↔ C++ ↔ drivers),
   - `--gc-sections` (paired with `-ffunction-sections`) to drop dead code.

### Rust: why the host target?

This machine ships rustc via the distro package with only the host target's
`std` — no `rustup`, so no `x86_64-unknown-none` prebuilts and no `-Zbuild-std`.
The crate therefore builds `#![no_std]` **against the host target**
(`x86_64-unknown-linux-gnu`, pinned in `kernel/rust/.cargo/config.toml`) while
never referencing `std`. It links cleanly because the final ELF uses
`-nostdlib`.

The intended production setup is a freestanding target. Two options:

- `rustup target add x86_64-unknown-none` (built-in, prebuilt `core`) — the
  least friction once rustup exists;
- a custom JSON target spec + nightly `-Zbuild-std` when we need precise
  control (e.g. `-mcmodel=kernel` for the higher-half remap).

Because `x86_64-unknown-linux-gnu` is an ABI-carrying target, LLVM may emit
SSE/AVX instructions — this is fine and expected; CR4.OSFXSR is set at boot
([boot.md](boot.md)). When we move to a freestanding target,
`kernel/rust/.cargo/config.toml` and the `PIPPIN_RUST_TARGET_SUBDIR` variable
in the root `CMakeLists.txt` must both change together.

## Reproducible detail: QEMU and the Multiboot address fields

QEMU <= 8.x rejects 64-bit ELF kernels through Multiboot (see the comment in
`boot.S`). We work around it with `MULTIBOOT_HEADER_HAS_ADDR` — the header
carries `load_addr`/`load_end_addr`/`bss_end_addr`/`entry_addr`, so the
bootloader byte-copies the image and never inspects ELF machine type. Keep
`__data_end`/`__bss_end` in the linker script consistent with the header.

## Debugging

- `make run-gdb`, then from another terminal:
  ```
  gdb build/kernel.elf
  (gdb) target remote :1234
  (gdb) break pippin_core_main
  (gdb) continue
  ```
- Watch the serial console for the Rust banner, memory info and the C++/driver
  counts — it is the integration test for the whole language bridge.
- `build/kernel.map` lists every linked symbol after GC.

## Adding a module (checklist)

1. Write the code in the right layer (see [kernel.md](kernel.md) "Adding
   something new").
2. Add it to its `CMakeLists.txt`. For a new Rust source, also add it to the
   `DEPENDS` list of the `rust-kernel` custom command.
3. Crossing a language boundary? Declare the `extern "C"` symbol in
   `kernel/rust/src/ffi.rs` and/or a `pippin/*.hh`/`*.h` header.
4. `make run` and confirm the new wiring shows up on the console.