# Drivers

The Driver layer is C++ (`drivers/cpp/`). It owns hardware: probing, device
trees, and the lifecycle of concrete drivers. It sits below the Toolbox API
and above the HAL, and is driven by the kernel core through C thunks.

## Driver ABI

Every driver is a C++ object derived from `pippin::drv::Driver`
(`drivers/cpp/include/pippin/drivers.hh`):

```cpp
class Driver {
   public:
    virtual ~Driver() = default;
    virtual const char* name() const = 0;   // "pci", "ps2", ...
    virtual bool probe() { return false; }  // "do I see my hardware?"
    virtual int  init()  { return 0; }      // bring it up
};
```

A Driver registry is maintained by the Driver Manager. The skeleton exports a
single `extern "C"` counting function (`pippin_driver_count()` in `pci.cc`)
which the Rust core calls during boot, proving the Rust→C++→registry path.

## Rules

- **Registration is static and declarative:** a driver registers by
  instantiating a registry entry at construction time; the Manager walks the
  registry after ACPI/PCI discovery. No dynamic lookup tables before Milestone 3.
- **Cross-language calls are `extern "C"`.** Rust calls the registry through
  `pippin_*` C names; it never sees C++ vtables. The vtables stay inside the
  Driver layer (`drivers/cpp/` + `kernel/cpp/`), which is exactly where they
  should.
- **Drivers may use the FPU/XMM** (the Milestone-2 scheduler saves per-task FP
  context); kernel core code still must not.
- **Probe ordering** follows the device tree (ACPI/PCI) rather than discovery
  order, so dependencies (e.g. a keyboard controller needing interrupt
  routing) can be satisfied. This arrives with Milestone 3.

## Roadmap (Milestone 3+)

| Driver    | Notes |
|-----------|-------|
| ACPI      | RSDP/SDT parsing in C++; hands the Manager bus topology + interrupt routing |
| PCI       | config-space walk on the PCI bus; stub target already exists (`pci.cc`) |
| PS/2      | keyboard/mouse; scancode → `kEventKey`/`kEventMouse` events |
| Serial    | 16550 stays the debug console; nothing special needed |
| VESA/EFI  | framebuffer via Limine (`boot/limine.conf` + `make-iso.sh`); feeds the Display Manager |
| Storage   | AHCI (SATA) skeleton; feeds the File Manager |
| Clock     | PIT → APIC timer; feeds the scheduler |

## Adding a driver

1. Create `drivers/cpp/src/<name>.cc` + a small header in
   `drivers/cpp/include/pippin/`.
2. Derive from `pippin::drv::Driver`, implement `name`/`probe`/`init`,
   register it, and wire a `pippin_<name>_*` C export if Rust needs to query it.
3. Add the source to `drivers/cpp/CMakeLists.txt`.
4. Keep the Manager's second: bump `pippin_driver_count()` output — the Rust
   boot banner prints it, so a working registration shows up in the serial log.