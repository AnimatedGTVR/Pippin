//! Numbered syscall ABI and ring-3 transition setup.

use core::fmt::Write;
use core::sync::atomic::{AtomicBool, Ordering};

use crate::{cpu, ipc, mem, mm, sched, serial};

const IA32_EFER: u32 = 0xC000_0080;
const IA32_STAR: u32 = 0xC000_0081;
const IA32_LSTAR: u32 = 0xC000_0082;
const IA32_FMASK: u32 = 0xC000_0084;
const USER_CODE: u64 = 0x0040_0000;
const USER_STACK: u64 = 0x0050_0000;
const USER_MMAP: u64 = 0x0060_0000;
static USER_MAPPED: AtomicBool = AtomicBool::new(false);

extern "C" {
    fn pippin_syscall_entry();
    static pippin_user_demo_start: u8;
    static pippin_user_demo_end: u8;
}

pub unsafe fn init() {
    // STAR: kernel CS=0x08; SYSRET's user base=0x20 gives SS=0x2b, CS=0x33.
    cpu::write_msr(IA32_STAR, (0x20u64 << 48) | (0x08u64 << 32));
    cpu::write_msr(IA32_LSTAR, pippin_syscall_entry as *const () as usize as u64);
    cpu::write_msr(IA32_FMASK, (1 << 9) | (1 << 10)); // mask IF and DF in kernel
    cpu::write_msr(IA32_EFER, cpu::read_msr(IA32_EFER) | 1); // SCE
}

fn user_range(ptr: u64, len: u64) -> bool {
    let Some(end) = ptr.checked_add(len) else { return false; };
    (ptr >= USER_CODE && end <= USER_CODE + 4096)
        || (ptr >= USER_STACK && end <= USER_STACK + 4096)
        || (USER_MAPPED.load(Ordering::Relaxed)
            && ptr >= USER_MMAP && end <= USER_MMAP + 4096)
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

/// Copy the small test program to a user page and enqueue its ring-3 thread.
pub fn spawn_demo(port: u16) -> bool {
    let start = unsafe { &pippin_user_demo_start as *const u8 };
    let end = unsafe { &pippin_user_demo_end as *const u8 };
    let len = end as usize - start as usize;
    if len == 0 || len > 4096 { return false; }
    let Some(code_phys) = mem::alloc_zeroed_frame() else { return false; };
    let Some(stack_phys) = mem::alloc_zeroed_frame() else { return false; };
    unsafe { core::ptr::copy_nonoverlapping(start, code_phys as *mut u8, len); }
    if mm::map_user_page(USER_CODE, code_phys).is_none()
        || mm::map_user_page(USER_STACK, stack_phys).is_none() {
        return false;
    }
    sched::spawn_user(USER_CODE, USER_STACK + 4096 - 8, port);
    true
}
