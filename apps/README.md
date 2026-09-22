# Apps

The C# M4 shell is in `csharp/`: `Pippin.UI` describes windows and widget trees;
`Pippin.Shell` defines separate panel, dock, launcher, settings, files,
notifications, wallpaper and sample app surfaces. Build and inspect them on the
host with `dotnet run --project csharp/Pippin.Shell/Pippin.Shell.csproj`.
From the repository root, `make run-shell` launches the interactive C# host
shell and displays its surfaces in QEMU.
`make run-ui` runs the newer window API: `Pippin.Window` is the low-level
client, `Pippin.Broker` multiplexes independent processes, `Pippin.UI`
provides the first layout and widget classes, and `Pippin.Examples` launches
About and Task Manager as separate processes.

The guest cannot run .NET code yet. These descriptions define the intended
client surface contract; a COM2 bridge now displays those surfaces in QEMU and
returns clicks to C#. The remaining application work arrives in this order:

- **Milestone 4.5 — C# desktop shell bridge:** host C# clients send separate
  shell windows to the Rust compositor.
- **Guest runtime:** a managed runtime, app loader and IPC move C# clients
  inside Pippin later.
- **Milestone 5 — native apps:** C++ and Rust GUI applications use the Toolbox
  API (Window/Menu/Control managers, event loop).
- **Milestone 6 — managed app expansion:** packaging and services for C# apps.

C# is the planned desktop shell language on x86-64. The kernel and compositor
remain freestanding Rust/C++ code. A possible 68k port needs a separate shell
decision.
Vanta is another candidate for native apps and utilities once its compiler can
target Pippin and use the Toolbox ABI; it has no scheduled milestone yet.
See docs/gui.md and docs/architecture.md (§ Application layer).
# M3 bundle

`demo/hello.pipb` is a small versioned data bundle. The File Manager validates
and reads it from a FAT32 AHCI test disk (`make run-disk`) or a Limine ISO module
(`make iso`). It is not yet an executable application; user app loading and the
Toolbox client API come later.
