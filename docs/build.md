# Build

Pippin builds two kernel ELFs from the same Rust core and C/C++ libraries:
`build/kernel.elf` for the direct QEMU Multiboot path and
`build/kernel-limine.elf` for the Limine protocol ISO. CMake drives both links;
the Makefile and shell scripts run the build, image, and emulator steps.

## Toolchain

This mandatory set, in the form installed on the current machine:

| Tool            | Version (tested)     | Use                             |
|-----------------|----------------------|---------------------------------|
| `cmake`         | 3.28                 | build orchestration             |
| `gcc`/`g++`     | 13.3                 | C, C++, and preprocessing `.S`  |
| `rustc`/`cargo` | stable with `x86_64-unknown-none` | freestanding Rust core |
| GNU `as`/`ld`   | binutils 2.42        | in-built assembler, final link  |
| `make`          | 4.3                  | top-level targets               |
| `bash`          | host shell           | QEMU and ISO helper scripts     |
| `qemu-system-x86_64` | 8.2 (optional)  | boot the OS without real HW     |
| Limine + `xorriso` | Limine 12.9 / xorriso 1.5.6 tested | hybrid BIOS/UEFI ISO |

`gcc` is used both as the C/C++ compiler and as the assembler driver
(`CMAKE_ASM_COMPILER` is pinned to it in the root `CMakeLists.txt`), so `.S`
files get preprocessed for free — no separate `nasm` needed. `clang` is not
required. Install the Rust target once with
`rustup target add x86_64-unknown-none`. Networking is not needed for later
builds.

## Building

```sh
make specs      # inspect host resources and calculate safe limits
make            # preflight + configure + compile build/kernel.elf (Multiboot)
make run        # Multiboot boot in a QEMU window; serial log in terminal
make run-shell  # alias for the native desktop
make run-ui     # alias for the native desktop
make run-headless # serial console only (also works without a desktop session)
make run-disk   # QEMU window with the FAT32 Hello bundle disk
make run-gdb    # QEMU paused, GDB stub on :1234
make iso        # build build/kernel-limine.elf and build/pippin.iso
qemu-system-x86_64 -machine q35 -m 256M -display gtk -serial stdio -cdrom build/pippin.iso
make clean      # rm build/
make distclean  # also wipe the cargo target dir
```

or raw CMake:

```sh
cmake -S . -B build -G "Unix Makefiles"
cmake --build build --target kernel.elf -j
qemu-system-x86_64 -machine q35 -m 256M -display gtk -serial stdio \
    -kernel build/kernel.elf
```

`make run` boots directly into the native graphical desktop. The shell model
is compiled as C++, exported through a small C ABI, and rendered by the Rust
compositor. Press Esc to return to the text command shell; run `desktop` to
reopen the graphics desktop. No host .NET process is required.

## Host preflight and resource limits

Before a normal kernel or ISO build, `scripts/preflight.sh` inspects the Linux
host and writes `build/pippin-limits.env`. The generated values include the
host logical-thread count, total RAM, free project-disk space, a resource
profile, a compiler job cap, and the QEMU guest-memory limit.

The current QEMU RAM policy is intentionally conservative:

| Host RAM | Profile | QEMU RAM |
| --- | --- | ---: |
| under 4 GiB | constrained | 256 MiB |
| 4-8 GiB | balanced | 384 MiB |
| 8-16 GiB | comfortable | 512 MiB |
| 16 GiB or more | high | 768 MiB |

Pippin's current early direct map covers the low 1 GiB, so the preflight never
allocates a guest-memory size above that architectural boundary. Build
parallelism reserves one logical host thread when possible and is capped at
eight jobs, with tighter caps on low-memory systems.

Run `make specs` to rerun and print the calculation without starting QEMU.

## Desktop runtime diagnostics

Graphical QEMU runs keep the launch terminal active as a diagnostic console.
The run script prefixes live output so the source is obvious:

- `[guest]` — Pippin COM1/serial kernel and shell output
- `[qemu]` — QEMU stderr/warnings
- `[qemu-debug]` — QEMU `guest_errors` diagnostics

The same session is written to `build/logs/runtime.log`. Dedicated copies live
at `build/logs/shell.log` and `build/logs/qemu-debug.log`. These files are
inside the ignored `build/` tree, so debug output does not pollute commits.

## How the languages meet

1. **Assembly** (`kernel/asm/boot.S`) — Multiboot header + long-mode bring-up.
   Compiled by the C compiler as an `ASM` source.
2. **Rust** (`kernel/rust`) — `cargo build --release` produces
   `libpippin_kernel.a` (a `staticlib`). The root `CMakeLists.txt` wraps that
   in a custom command/target (`rust-kernel`) and feeds the archive to the
   final link.
3. **C++ + C + drivers** — ordinary CMake `STATIC` libraries.
4. **Final links** (`kernel/CMakeLists.txt`) — each image uses one `ld` pass with
   - `-nostdlib`, `-no-pie` (plain `ET_EXEC`, required for Multiboot),
   - `kernel/linker.ld` for Multiboot or `kernel/limine-linker.ld` for Limine,
   - all archives wrapped in `-Wl,--start-group/--end-group` because they
     reference each other in both directions (Rust ↔ C++ ↔ drivers),
   - `--gc-sections` (paired with `-ffunction-sections`) to drop dead code.
5. **Shell glue** (`scripts/run-qemu.sh`, `scripts/make-iso.sh`,
   `scripts/make-fat-image.sh`) — runs QEMU and
   image creation after the ELF is built. These scripts run on the host and do
   not add shell code to the kernel or application runtime.

### Freestanding Rust target

`kernel/rust/.cargo/config.toml` selects `x86_64-unknown-none`, static
relocations, and the large code model. The root `CMakeLists.txt` names the same
target directory when it links the Rust static library. Both kernel images
share that library. The source remains `#![no_std]` and dependency-free.

The Multiboot trampoline enables SSE before Rust starts; Limine hands off with
SSE available. Kernel code avoids floating point; the scheduler saves task
FP/XMM state during switches.

### M3 storage check

`make run-disk` builds a 64 MiB FAT32 image containing `apps/demo/hello.pipb`
and boots QEMU with an AHCI SATA disk. The serial banner should show a readable
sector 0 and the Hello bundle loaded through Pippin's File Manager. `make disk`
only rebuilds the test image.

### Limine ISO prerequisites

Install Limine's BIOS/UEFI assets and `limine` installer, plus `xorriso`.
`scripts/make-iso.sh` reads Limine assets from `/usr/local/share/limine` by
default. Set `LIMINE_DIR`, `LIMINE_BIN`, and `XORRISO_BIN` to use other paths.
The tested setup used the Limine 12.9.0 binary release. `make iso` builds the
Limine ELF, stages the files, creates the hybrid ISO, and runs `limine
bios-install`.

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
