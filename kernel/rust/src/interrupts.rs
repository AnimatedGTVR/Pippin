//! Interrupt dispatch, 8259A PIC and the PIT system timer (1000 Hz).
//!
//! Dispatch is intentionally small at M1: exceptions fault out loudly,
//! IRQ0 advances the tick counter, and everything else is logged. Their
//! handler tables grow as the scheduler, IPC and syscalls arrive.

use core::fmt::Write;
use core::sync::atomic::{AtomicU64, Ordering};

use crate::apic;
use crate::cpu;
use crate::idt;
use crate::serial;

/// System tick counter, advanced by the PIT at 1000 Hz.
static TICKS: AtomicU64 = AtomicU64::new(0);

// ---------------------------------------------------------------------------
// 8259A PIC
// ---------------------------------------------------------------------------

const PIC1_CMD: u16 = 0x20;
const PIC1_DATA: u16 = 0x21;
const PIC2_CMD: u16 = 0xA0;
const PIC2_DATA: u16 = 0xA1;
const PIC_EOI: u8 = 0x20;

/// Remap the PIC to vectors 0x20..0x2F so IRQs never collide with CPU
/// exceptions, then mask everything off.
///
/// # Safety
/// Port I/O; call once before `idt::init` and `sti`.
unsafe fn pic_init() {
    cpu::outb(PIC1_CMD, 0x11); // init, will expect ICW2-4
    cpu::outb(PIC2_CMD, 0x11);
    cpu::outb(PIC1_DATA, 0x20); // master IRQ base 0x20
    cpu::outb(PIC2_DATA, 0x28); // slave IRQ base 0x28
    cpu::outb(PIC1_DATA, 0x04); // slave cascade on master IRQ2
    cpu::outb(PIC2_DATA, 0x02); // slave cascade identity
    cpu::outb(PIC1_DATA, 0x01); // 8086 mode, normal EOI
    cpu::outb(PIC2_DATA, 0x01);
    cpu::outb(PIC1_DATA, 0xFF); // mask everything
    cpu::outb(PIC2_DATA, 0xFF);
}

/// Enable (unmask) one IRQ line.
///
/// # Safety
/// Port I/O.
pub unsafe fn pic_unmask(irq: u8) {
    let (data, mask) = if irq < 8 { (PIC1_DATA, 1u8 << irq) } else { (PIC2_DATA, 1u8 << (irq - 8)) };
    let cur = cpu::inb(data);
    cpu::outb(data, cur & !mask);
}

/// Send the end-of-interrupt to the PIC(s).
///
/// # Safety
/// Port I/O; must be called from the IRQ handler owning this line.
pub unsafe fn pic_eoi(irq: u8) {
    if irq >= 8 {
        cpu::outb(PIC2_CMD, PIC_EOI);
    }
    cpu::outb(PIC1_CMD, PIC_EOI);
}

// ---------------------------------------------------------------------------
// PIT (8254) — system timer at 1000 Hz
// ---------------------------------------------------------------------------

const PIT_CMD: u16 = 0x43;
const PIT_CH0: u16 = 0x40;
const PIT_FREQ_HZ: u64 = 1_193_182; // base 1.19318 MHz
const TICKS_PER_SEC: u64 = 1000;

/// Program channel 0 as a square wave at `hz`, binary counting.
///
/// # Safety
/// Port I/O.
unsafe fn pit_init(hz: u64) {
    let divisor: u16 = (PIT_FREQ_HZ / hz) as u16;
    cpu::outb(PIT_CMD, 0x36); // ch0, lobyte/hibyte, mode 3, binary
    cpu::outb(PIT_CH0, (divisor & 0xFF) as u8);
    cpu::outb(PIT_CH0, (divisor >> 8) as u8);
}

// ---------------------------------------------------------------------------
// Public init + query
// ---------------------------------------------------------------------------

/// Interrupt subsystem bootstrap.
///
/// # Safety
/// Called once from the boot path after the heap is ready.
pub unsafe fn init() {
    pic_init();
    pit_init(TICKS_PER_SEC);
    idt::init();
    pic_unmask(0); // PIT timer
    cpu::sti();
}

#[inline]
pub fn ticks() -> u64 {
    TICKS.load(Ordering::Relaxed)
}

/// Busy-wait on the timer; raises the CPU's IF simultaneously so a HLT-during-
/// wait does not deadlock.
pub fn sleep_ms(ms: u64) {
    let target = TICKS.load(Ordering::Relaxed) + ms;
    while TICKS.load(Ordering::Relaxed) < target {
        cpu::halt();
    }
}

// ---------------------------------------------------------------------------
// Dispatch
// ---------------------------------------------------------------------------

/// Common entry point referenced from isr.S. `frame` is the normalized
/// interrupt frame; do not let it escape this function.
#[no_mangle]
pub unsafe extern "C" fn pippin_int_dispatch(frame: *mut idt::IntFrame) {
    let vector = (*frame).vector as u8;
    match vector {
        // Breakpoint: INT3 already pushed the following instruction's address,
        // so a plain return resumes cleanly and the breakpoint can re-fire.
        3 => {
            if serial::is_ready() {
                let mut c = serial::stdout();
                let _ = writeln!(c, "  int: #BP breakpoint caught");
            }
        }
        // CPU exceptions other than the breakpoint: log and stop.
        0..=31 => fault(vector, &*frame),
        // System timer.
        32 => {
            if !apic::active() {
                TICKS.fetch_add(1, Ordering::Relaxed);
            }
            pic_eoi(0);
        }
        0x30 => {
            TICKS.fetch_add(1, Ordering::Relaxed);
            apic::eoi();
        }
        0xFF => {} // local APIC spurious vector needs no EOI
        // Remaining hardware IRQs: unmapped for now, log + ack.
        33..=47 => {
            let irq = vector - 32;
            if serial::is_ready() {
                let mut c = serial::stdout();
                let _ = writeln!(c, "IRQ {} unhandled", irq);
            }
            pic_eoi(irq);
        }
        v => log_fatal(frame, v, "unhandled interrupt"),
    }
}

/// Dead-end for CPU exceptions: report the frame, then hang.
unsafe fn fault(vector: u8, frame: &idt::IntFrame) -> ! {
    let names = [
        "#DE divide by zero", "#DB debug", "#NMI non-maskable", "#BP breakpoint",
        "#OF overflow", "#BR bound range", "#UD invalid opcode", "#NM device not avail",
        "#DF double fault", "#CO coproc overrun", "#TS invalid TSS", "#NP segment missing",
        "#SS stack fault", "#GP general protection", "#PF page fault", "#15 reserved",
        "#MF fpu error", "#AC alignment check", "#MC machine check", "#XF simd",
    ];
    let what = names.get(vector as usize).copied().unwrap_or("exception");
    log_fatal(frame as *const idt::IntFrame as *mut idt::IntFrame, vector, what);
}

unsafe fn log_fatal(frame: *mut idt::IntFrame, vector: u8, what: &str) -> ! {
    if serial::is_ready() {
        let mut c = serial::stdout();
        let _ = write!(c, "FATAL {} (vector {}): ", what, vector);
        let _ = write!(c, "rip={:#x} cs={:#x} rflags={:#x} err={:#x}",
            (*frame).rip, (*frame).cs, (*frame).rflags, (*frame).err);
        let _ = writeln!(c);
    }
    loop {
        cpu::halt();
    }
}
