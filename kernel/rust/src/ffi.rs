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
}

/// Read the C-string version banner exported by the C++ runtime.
pub fn cpp_version() -> &'static str {
    // SAFETY: pippin_cpp_version returns a pointer to a NUL-terminated
    // static string that lives for the lifetime of the kernel.
    unsafe { cstr_to_str(pippin_cpp_version()) }
}

/// Query the (currently zero) driver count.
pub fn driver_count() -> u32 {
    // SAFETY: trivially safe C call.
    unsafe { pippin_driver_count() }
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