//! Local APIC timer, calibrated against the PIT during bootstrap.

use core::sync::atomic::{AtomicBool, Ordering};

use crate::{cpu, interrupts, mm};

const IA32_APIC_BASE: u32 = 0x1B;
const APIC_ENABLE: u64 = 1 << 11;
const APIC_BASE_MASK: u64 = 0xFFFF_F000;
const REG_EOI: usize = 0x0B0;
const REG_SVR: usize = 0x0F0;
const REG_LVT_TIMER: usize = 0x320;
const REG_TIMER_INITIAL: usize = 0x380;
const REG_TIMER_CURRENT: usize = 0x390;
const REG_TIMER_DIVIDE: usize = 0x3E0;
const TIMER_VECTOR: u32 = 0x30;
const SPURIOUS_VECTOR: u32 = 0xFF;

static mut APIC_VIRT: *mut u8 = core::ptr::null_mut();
static ACTIVE: AtomicBool = AtomicBool::new(false);

unsafe fn read(reg: usize) -> u32 {
    core::ptr::read_volatile(APIC_VIRT.add(reg) as *const u32)
}

unsafe fn write(reg: usize, value: u32) {
    core::ptr::write_volatile(APIC_VIRT.add(reg) as *mut u32, value);
    let _ = read(REG_SVR); // drain posted MMIO write
}

pub fn active() -> bool {
    ACTIVE.load(Ordering::Relaxed)
}

/// Acknowledge a local APIC interrupt.
pub unsafe fn eoi() {
    write(REG_EOI, 0);
}

/// Start a 1000 Hz local APIC timer using PIT ticks for calibration.
/// Returns false on CPUs without an APIC or if its MMIO page cannot be mapped.
///
/// # Safety
/// The IDT and PIT must already be running; call once on the boot CPU.
pub unsafe fn init_timer(map_mmio: bool) -> bool {
    let cpuid = core::arch::x86_64::__cpuid(1);
    if cpuid.edx & (1 << 9) == 0 {
        return false;
    }
    let base_msr = cpu::read_msr(IA32_APIC_BASE);
    let phys = base_msr & APIC_BASE_MASK;
    APIC_VIRT = if map_mmio {
        match mm::map_mmio_page(phys) {
            Some(p) => p,
            None => return false,
        }
    } else {
        // Limine base revision 0 provides an identity map through 4 GiB.
        phys as *mut u8
    };
    cpu::write_msr(IA32_APIC_BASE, base_msr | APIC_ENABLE);
    write(REG_SVR, read(REG_SVR) | (1 << 8) | SPURIOUS_VECTOR);
    write(REG_TIMER_DIVIDE, 0x3); // divide bus clock by 16
    write(REG_LVT_TIMER, TIMER_VECTOR | (1 << 16)); // mask during calibration
    write(REG_TIMER_INITIAL, u32::MAX);

    let start = interrupts::ticks();
    while interrupts::ticks().wrapping_sub(start) < 20 {
        cpu::halt();
    }
    let elapsed = u32::MAX - read(REG_TIMER_CURRENT);
    let per_ms = elapsed / 20;
    if per_ms < 100 {
        write(REG_LVT_TIMER, TIMER_VECTOR | (1 << 16));
        return false;
    }

    // The PIT has supplied time until this point. Switch the time source to
    // the calibrated local APIC, then mask legacy IRQ0 at the PIC.
    ACTIVE.store(true, Ordering::Relaxed);
    write(REG_LVT_TIMER, TIMER_VECTOR | (1 << 17)); // periodic
    write(REG_TIMER_INITIAL, per_ms);
    cpu::outb(0x21, cpu::inb(0x21) | 1);
    true
}
