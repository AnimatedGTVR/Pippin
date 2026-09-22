//! Foreign-function boundary between the Rust core and the C++/C side.
//!
//! All cross-language calls go through plain C symbols (`extern "C"`).
//! C++ never calls mangled Rust functions and Rust never touches C++ ABIs
//! directly; the boundary is documented in docs/architecture.md (§ FFI).

use core::slice;
use core::str;

extern "C" {
    /// Version banner from the C++ kernel runtime.
    fn pippin_cpp_version() -> *const u8;

    /// Number of drivers registered with the Driver Manager (C++ side).
    fn pippin_driver_count() -> u32;
    fn pippin_driver_probe_all() -> u32;
    fn pippin_pci_device_count() -> u32;
    fn pippin_pci_parent(index: u32) -> i32;
    fn pippin_pci_device(index: u32, vendor: *mut u16, product: *mut u16,
                         class_code: *mut u8, subclass: *mut u8) -> bool;
    fn pippin_ahci_bar() -> u64;
    fn pippin_qemu_vga_bar() -> u64;
}

/// Read the C-string version banner exported by the C++ runtime.
pub fn cpp_version() -> &'static str {
    // SAFETY: pippin_cpp_version returns a pointer to a NUL-terminated
    // static string that lives for the lifetime of the kernel.
    unsafe { cstr_to_str(pippin_cpp_version()) }
}

/// Query the initialized driver count.
pub fn driver_count() -> u32 {
    // SAFETY: trivially safe C call.
    unsafe { pippin_driver_count() }
}

pub fn probe_drivers() -> u32 { unsafe { pippin_driver_probe_all() } }
pub fn pci_device_count() -> u32 { unsafe { pippin_pci_device_count() } }
pub fn pci_parent(index: u32) -> i32 { unsafe { pippin_pci_parent(index) } }
pub fn ahci_bar() -> u64 { unsafe { pippin_ahci_bar() } }
pub fn qemu_vga_bar() -> u64 { unsafe { pippin_qemu_vga_bar() } }
pub fn pci_device(index: u32) -> Option<(u16, u16, u8, u8)> {
    let (mut vendor, mut product, mut class, mut subclass) = (0, 0, 0, 0);
    if unsafe { pippin_pci_device(index, &mut vendor, &mut product, &mut class, &mut subclass) } {
        Some((vendor, product, class, subclass))
    } else { None }
}

/// Convert a borrowed, NUL-terminated C string to `&str`.
unsafe fn cstr_to_str(mut ptr: *const u8) -> &'static str {
    if ptr.is_null() {
        return "<?>";
    }
    let mut len = 0usize;
    while unsafe { ptr.read() } != 0 {
        len += 1;
        ptr = unsafe { ptr.add(1) };
    }
    // SAFETY: validated NUL-terminated range of len bytes.
    let bytes = unsafe { slice::from_raw_parts(ptr.sub(len), len) };
    str::from_utf8(bytes).unwrap_or("<!>")
}
