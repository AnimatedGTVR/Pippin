# Apps

Pippin's desktop shell is now native C++. The current shell model is compiled
from `kernel/cpp/src/shell.cc`, crosses a deliberately small C ABI in
`kernel/c/include/pippin/shell.h`, and is consumed by the Rust compositor.

The built-in shell currently defines the panel, dock, launcher, Files, Settings,
and Terminal surfaces. Panel and dock start visible; the remaining surfaces are
created hidden and restored directly inside Pippin when their shell actions fire.
No host .NET runtime or COM2 C# bridge is required for the default desktop.

The previous C# experiments remain under `apps/csharp/` as a design/prototyping
reference while the native C++ toolkit and application layer are developed.
They are not built by `make run`.

The native application work now proceeds in this order:

- **Native shell model:** C++ owns desktop surface definitions and shell behavior.
- **C ABI boundary:** C keeps the cross-language contract simple and stable.
- **Rust compositor:** Rust validates, owns, composites, and routes input.
- **C++ UI toolkit:** reusable Button, Label, Stack, Card, Icon, TextField, etc.
- **Native ELF loader:** static x86-64 `ET_EXEC` C++ apps now load into mapped ring-3 memory with a real user stack.
- **Apps:** Files, Settings, Terminal, and future apps use the same C++ toolkit.

Vanta can target the same Toolbox/C ABI later without changing the compositor.

# M3 bundle

`demo/hello.pipb` is a small versioned data bundle. The File Manager validates
and reads it from a FAT32 AHCI test disk (`make run-disk`) or a Limine ISO module
(`make iso`). It is still a data bundle, while executable apps now use the ELF64 path documented in `docs/apps.md`.


## Native executable sample

`native/hello.cpp` builds to `build/apps/native/pippin-hello.elf`. Use `make apps` to build it directly. Pippin embeds that ELF for the bootstrap integration test and can launch it from the serial shell with `run hello`.
