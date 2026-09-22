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
    ~Driver() = default;                  // static lifetime; never deleted
    virtual const char* name() const = 0;   // "pci", "ps2", ...
    virtual bool probe() { return false; }  // "do I see my hardware?"
    virtual int  init()  { return 0; }      // bring it up
};
```

A static Driver registry is maintained by the Driver Manager. At boot the Rust
core calls `probe` and `init` through C thunks. The PCI driver scans bus,
device and function configuration space, records device class IDs and links
devices behind bridges to their nearest parent bridge.

## Rules

- **Registration is static:** the Manager has a fixed driver list and runs
  `probe` then `init` at boot.
- **Cross-language calls are `extern "C"`.** Rust calls the registry through
  `pippin_*` C names; it never sees C++ vtables. The vtables stay inside the
  Driver layer (`drivers/cpp/` + `kernel/cpp/`), which is exactly where they
  should.
- **Drivers may use the FPU/XMM** (the Milestone-2 scheduler saves per-task FP
  context); kernel core code still must not.
- **PCI discovery** uses configuration mechanism 1 and records bridge parent
  relationships. ACPI RSDP/root-table validation precedes broader namespace
  and interrupt-routing support.

## Driver status

| Driver    | Notes |
|-----------|-------|
| ACPI      | RSDP and RSDT/XSDT header/checksum validated; namespace work later |
| PCI       | config-space scan, class IDs and bridge parent tree |
| PS/2      | polled keyboard/mouse bytes → Event Manager IPC events |
| Serial    | 16550 stays the debug console; nothing special needed |
| Display  | 32-bit RGB framebuffer via Limine; QEMU VGA linear framebuffer preview |
| Storage   | polling AHCI SATA reader; read-only FAT32 Hello bundle |
| Clock     | PIT → APIC timer; feeds the scheduler |

## Adding a driver

1. Create `drivers/cpp/src/<name>.cc` + a small header in
   `drivers/cpp/include/pippin/`.
2. Derive from `pippin::drv::Driver`, implement `name`/`probe`/`init`,
   register it, and wire a `pippin_<name>_*` C export if Rust needs to query it.
3. Add the source to `drivers/cpp/CMakeLists.txt`.
4. Add the driver to the static registry in `pci.cc` or its successor Manager
   source, then check the boot banner's active count.
