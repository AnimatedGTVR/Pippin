# Native applications

Pippin now has a bootstrap ELF64 loader for native x86-64 applications.

## Supported ELF format

The loader accepts:

- ELF64, little-endian
- x86-64 (`EM_X86_64`)
- static `ET_EXEC` images
- up to 16 program headers
- `PT_LOAD` segments inside the reserved user application window
- zero-filled BSS where `p_memsz > p_filesz`

Dynamic linking, PIE relocation, shared libraries, TLS, and ELF interpreters are
not supported yet.

Native applications are linked above the kernel's low 1 GiB direct-map window:

- application range: `0x0000000100000000..0x0000000102000000`
- user stack top: `0x0000000104000000`
- syscall mmap page: `0x0000000108000000`

This keeps user mappings from replacing the low identity/direct mappings that
the kernel uses to access physical frames.

## Loader path

`kernel/rust/src/elf.rs` validates and loads an ELF image. For each
`PT_LOAD` segment it:

1. validates address/file bounds and alignment
2. allocates zeroed physical frames
3. creates user-accessible page mappings
4. copies file-backed bytes into those frames
5. leaves the remaining segment bytes zeroed for BSS
6. allocates a 16 KiB user stack
7. enters ring 3 at `e_entry`

If loading fails, mapped application frames are rolled back. After the process
exits, its mappings are reclaimed before the next app launch.

Syscalls validate user pointers against the actual page tables rather than
trusting a fixed hardcoded address range.

## First C++ application

`apps/native/hello.cpp` is a freestanding C++20 app linked with
`apps/native/user.ld`. It has no libc, runtime, dynamic linker, exceptions, or
RTTI. It proves that a loaded C++ ELF can:

- enter ring 3
- receive the boot event-port ID
- send an IPC event
- log a string from ELF `.rodata`
- exit through the Pippin syscall ABI

Build the native app directly with:

```sh
make apps
```

The resulting ELF is:

```text
build/apps/native/pippin-hello.elf
```

A generated assembly wrapper embeds that ELF in the kernel image for the
bootstrap test. The loader API itself accepts any byte slice, so storage-backed
ELFs can use the same path later.

From the Pippin serial command shell:

```text
apps
run hello
```

## Current limitation

Pippin still uses one shared kernel page-table root and currently reserves one
scheduler slot for a native user application. The loader is therefore a real
ring-3 ELF loader, but not yet a multi-process address-space implementation.

The next major step is per-process page tables / CR3 switching. Once that exists,
the C++ shell can move out of the kernel image and become a normal loaded Pippin
application.
