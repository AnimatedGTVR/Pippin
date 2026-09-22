//! x86-64 CPU primitives: port I/O and the idle halt.

use core::arch::asm;

/// Write a byte to an I/O port.
#[inline]
pub unsafe fn outb(port: u16, value: u8) {
    asm!(
        "out dx, al",
        in("dx") port,
        in("al") value,
        options(nomem, nostack, preserves_flags)
    );
}

/// Read a byte from an I/O port.
#[inline]
pub unsafe fn inb(port: u16) -> u8 {
    let value: u8;
    asm!(
        "in al, dx",
        out("al") value,
        in("dx") port,
        options(nomem, nostack, preserves_flags)
    );
    value
}

/// Write a 16-bit word to an I/O port.
#[inline]
pub unsafe fn outw(port: u16, value: u16) {
    asm!("out dx, ax", in("dx") port, in("ax") value,
         options(nomem, nostack, preserves_flags));
}

/// Read a 16-bit word from an I/O port.
#[inline]
pub unsafe fn inw(port: u16) -> u16 {
    let value: u16;
    asm!("in ax, dx", out("ax") value, in("dx") port,
         options(nomem, nostack, preserves_flags));
    value
}

/// Halt the current CPU until the next interrupt.
///
/// Safe to call with interrupts enabled or disabled; with interrupts
/// disabled this is the idle/panic loop.
#[inline]
pub fn halt() {
    unsafe {
        asm!("hlt", options(nomem, nostack, preserves_flags));
    }
}

/// Disable interrupts.
#[inline]
#[allow(dead_code)] // used during higher-half transition
pub unsafe fn cli() {
    asm!("cli", options(nomem, nostack, preserves_flags));
}

/// Enable interrupts.
#[inline]
pub unsafe fn sti() {
    asm!("sti", options(nomem, nostack, preserves_flags));
}

/// Install a new PML4 by writing the physical address into CR3.
///
/// # Safety
/// `pml4_phys` must point at a valid page-table root that maps the code
/// that continues executing (both the low identity window and the
/// higher-half kernel must stay reachable during the switch).
#[inline]
pub unsafe fn install_paging(pml4_phys: u64) {
    // `mov` to CR3 also flushes the TLB, acting as the paging switch. `rax` is
    // pinned explicitly (Intel syntax is the asm! default; CR moves need the
    // literal register token).
    asm!("mov cr3, rax", in("rax") pml4_phys, options(nomem, nostack, preserves_flags));
}

/// Read the current page-table root (CR3).
#[inline]
pub fn read_cr3() -> u64 {
    let value: u64;
    unsafe {
        asm!("mov rax, cr3", out("rax") value, options(nomem, nostack, preserves_flags));
    }
    value
}

#[inline]
pub unsafe fn read_msr(msr: u32) -> u64 {
    let lo: u32;
    let hi: u32;
    asm!("rdmsr", in("ecx") msr, out("eax") lo, out("edx") hi, options(nomem, nostack));
    ((hi as u64) << 32) | lo as u64
}

#[inline]
pub unsafe fn write_msr(msr: u32, value: u64) {
    asm!("wrmsr", in("ecx") msr, in("eax") value as u32,
         in("edx") (value >> 32) as u32, options(nomem, nostack));
}

/// Save RFLAGS and mask interrupts for a short single-CPU critical section.
pub fn irq_save() -> u64 {
    let flags: u64;
    unsafe {
        asm!("pushfq", "pop {}", out(reg) flags, options(preserves_flags));
        cli();
    }
    flags
}

/// Restore the previous interrupt-enabled state.
pub fn irq_restore(flags: u64) {
    if flags & (1 << 9) != 0 {
        unsafe { sti(); }
    }
}
