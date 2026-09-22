//! Interrupt Descriptor Table and the interrupt stack frame.
//!
//! The stubs in `kernel/asm/isr.S` normalize every interrupt to one frame
//! shape and funnel them into `interrupts::dispatch`. This module only builds
//! and installs the table; actual handling lives in `interrupts.rs`.

/// Kernel code segment from boot.S's GDT.
pub const GDT_CODE: u16 = 0x08;

/// A 64-bit interrupt gate.
const GATE_INTERRUPT: u8 = 0x0E;

/// One 16-byte IDT descriptor.
#[repr(C)]
#[derive(Clone, Copy)]
struct IdtEntry {
    offset_low: u16,
    selector: u16,
    ist: u8,
    flags: u8,
    offset_mid: u16,
    offset_high: u32,
    reserved: u32,
}

impl IdtEntry {
    const fn empty() -> Self {
        Self { offset_low: 0, selector: 0, ist: 0, flags: 0, offset_mid: 0, offset_high: 0, reserved: 0 }
    }

    const fn new(offset: u64, selector: u16, dpl: u8, gate: u8, ist: u8) -> Self {
        Self {
            offset_low: (offset & 0xFFFF) as u16,
            selector,
            ist,
            flags: 0x80 | (dpl << 5) | gate, // present | dpl | gate type
            offset_mid: ((offset >> 16) & 0xFFFF) as u16,
            offset_high: (offset >> 32) as u32,
            reserved: 0,
        }
    }

    #[allow(dead_code)] // diagnostic helper for dump/tests
    fn offset(&self) -> u64 {
        u64::from(self.offset_low)
            | (u64::from(self.offset_mid) << 16)
            | (u64::from(self.offset_high) << 32)
    }
}

/// IDT descriptor loaded into the IDTR.
#[repr(C, packed)]
struct Idtr {
    limit: u16,
    base: u64,
}

// Stub address table emitted by isr.S: index = interrupt vector.
extern "C" {
    #[link_name = "isr_stub_table"]
    static ISR_STUB_TABLE: [usize; 256];
}

static mut IDT: [IdtEntry; 256] = [IdtEntry::empty(); 256];

/// Interrupt frame presented to the dispatcher, low to high addresses,
/// matching the isr.S prologue exactly.
#[repr(C)]
pub struct IntFrame {
    pub r15: u64,
    pub r14: u64,
    pub r13: u64,
    pub r12: u64,
    pub r11: u64,
    pub r10: u64,
    pub r9: u64,
    pub r8: u64,
    pub rbp: u64,
    pub rdi: u64,
    pub rsi: u64,
    pub rdx: u64,
    pub rcx: u64,
    pub rbx: u64,
    pub rax: u64,
    pub vector: u64,
    pub err: u64,
    pub rip: u64,
    pub cs: u64,
    pub rflags: u64,
    pub rsp: u64,
    pub ss: u64,
}

/// Fill every IDT slot with the matching stub and load the table.
///
/// # Safety
/// Called once from the boot path, before interrupts are enabled.
pub unsafe fn init() {
    for (i, slot) in IDT.iter_mut().enumerate() {
        let stub = ISR_STUB_TABLE[i] as u64;
        let ist = if i == 8 { crate::gdt::double_fault_ist() } else { 0 };
        *slot = IdtEntry::new(stub, GDT_CODE, 0, GATE_INTERRUPT, ist);
    }
    let idtr = Idtr {
        limit: (core::mem::size_of_val(&IDT) - 1) as u16,
        base: (&IDT as *const IdtEntry) as u64,
    };
    core::arch::asm!("lidt [{}]", in(reg) &idtr, options(nostack, preserves_flags));
}

impl core::fmt::Debug for IntFrame {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("IntFrame")
            .field("vector", &self.vector)
            .field("err", &self.err)
            .field("rip", &self.rip)
            .field("cs", &self.cs)
            .field("rflags", &self.rflags)
            .finish()
    }
}
