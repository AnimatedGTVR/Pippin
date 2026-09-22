//! Numbered syscall ABI and ring-3 transition setup.

use core::fmt::Write;
use core::sync::atomic::{AtomicBool, Ordering};

use crate::{cpu, ipc, mem, mm, sched, serial};

const IA32_EFER: u32 = 0xC000_0080;
const IA32_STAR: u32 = 0xC000_0081;
const IA32_LSTAR: u32 = 0xC000_0082;
const IA32_FMASK: u32 = 0xC000_0084;
const USER_MMAP: u64 = 0x0000_0001_0800_0000;
static USER_MAPPED: AtomicBool = AtomicBool::new(false);

extern "C" {
    fn pippin_syscall_entry();
}

pub unsafe fn init() {
    // STAR: kernel CS=0x08; SYSRET's user base=0x20 gives SS=0x2b, CS=0x33.
    cpu::write_msr(IA32_STAR, (0x20u64 << 48) | (0x08u64 << 32));
    cpu::write_msr(IA32_LSTAR, pippin_syscall_entry as *const () as usize as u64);
    cpu::write_msr(IA32_FMASK, (1 << 9) | (1 << 10)); // mask IF and DF in kernel
    cpu::write_msr(IA32_EFER, cpu::read_msr(IA32_EFER) | 1); // SCE
}

fn user_range(ptr: u64, len: u64) -> bool {
    mm::user_range_mapped(ptr, len)
}

/// Dispatch arguments use RAX for the number and RDI, RSI, RDX for three
/// arguments. Errors return `u64::MAX`.
#[no_mangle]
pub extern "C" fn pippin_syscall_dispatch(number: u64, a0: u64, a1: u64, a2: u64) -> u64 {
    match number {
        0 => sched::exit_current(),
        1 => {
            if a1 > 256 || !user_range(a0, a1) { return u64::MAX; }
            let bytes = unsafe { core::slice::from_raw_parts(a0 as *const u8, a1 as usize) };
            if let Ok(message) = core::str::from_utf8(bytes) {
                let _ = serial::stdout().write_str(message);
                0
            } else { u64::MAX }
        }
        2 => {
            if a0 != USER_MMAP || USER_MAPPED.load(Ordering::Relaxed) { return u64::MAX; }
            let Some(phys) = mem::alloc_zeroed_frame() else { return u64::MAX; };
            if mm::map_user_page(USER_MMAP, phys).is_none() {
                mem::free_frames(phys, 1);
                return u64::MAX;
            }
            USER_MAPPED.store(true, Ordering::Relaxed);
            USER_MMAP
        }
        3 => if a0 <= u16::MAX as u64 && a1 <= u16::MAX as u64
                    && ipc::send(a0 as u16, a1 as u16, a2) { 0 } else { u64::MAX },
        4 => {
            if !user_range(a1, core::mem::size_of::<ipc::Event>() as u64) { return u64::MAX; }
            if let Some(event) = ipc::recv(a0 as u16) {
                unsafe { core::ptr::write_unaligned(a1 as *mut ipc::Event, event); }
                0
            } else { u64::MAX }
        }
        _ => u64::MAX,
    }
}

