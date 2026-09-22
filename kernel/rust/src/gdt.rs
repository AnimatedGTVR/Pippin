//! Kernel GDT and 64-bit task-state segment.

use core::arch::asm;

const TSS_SELECTOR: u16 = 0x18;
const DOUBLE_FAULT_IST: u8 = 1;

#[repr(C, packed)]
struct Tss {
    reserved0: u32,
    rsp: [u64; 3],
    reserved1: u64,
    ist: [u64; 7],
    reserved2: u64,
    reserved3: u16,
    iomap_base: u16,
}

#[repr(C, packed)]
struct Gdtr {
    limit: u16,
    base: u64,
}

static mut GDT: [u64; 5] = [0; 5];
static mut TSS: Tss = Tss {
    reserved0: 0,
    rsp: [0; 3],
    reserved1: 0,
    ist: [0; 7],
    reserved2: 0,
    reserved3: 0,
    iomap_base: core::mem::size_of::<Tss>() as u16,
};
static mut DOUBLE_FAULT_STACK: [u8; 16384] = [0; 16384];

/// Install ring-0 selectors and a TSS with a dedicated double-fault stack.
///
/// # Safety
/// Call once on the boot CPU before loading the IDT or enabling interrupts.
pub unsafe fn init() {
    let rsp: u64;
    asm!("mov {}, rsp", out(reg) rsp, options(nomem, nostack, preserves_flags));
    TSS.rsp[0] = rsp;
    TSS.ist[0] = DOUBLE_FAULT_STACK.as_ptr().add(DOUBLE_FAULT_STACK.len()) as u64;

    GDT[0] = 0;
    GDT[1] = 0x00AF_9A00_0000_FFFF; // 64-bit ring-0 code, selector 0x08
    GDT[2] = 0x00CF_9200_0000_FFFF; // ring-0 data, selector 0x10

    let base = &TSS as *const Tss as u64;
    let limit = (core::mem::size_of::<Tss>() - 1) as u64;
    GDT[3] = (limit & 0xFFFF)
        | ((base & 0xFF_FFFF) << 16)
        | (0x89u64 << 40) // present, available 64-bit TSS
        | ((limit & 0xF0000) << 32)
        | (((base >> 24) & 0xFF) << 56);
    GDT[4] = base >> 32;

    let gdtr = Gdtr {
        limit: (core::mem::size_of_val(&GDT) - 1) as u16,
        base: GDT.as_ptr() as u64,
    };
    asm!("lgdt [{}]", in(reg) &gdtr, options(readonly, nostack, preserves_flags));
    // Limine enters with CS=0x28 and data selectors=0x30. Reload both after
    // installing our smaller GDT, before any interrupt can use its gates.
    asm!(
        "push 0x08",
        "lea rax, [rip + 2f]",
        "push rax",
        "retfq",
        "2:",
        "mov ax, 0x10",
        "mov ds, ax",
        "mov es, ax",
        "mov ss, ax",
        "mov fs, ax",
        "mov gs, ax",
        out("rax") _,
    );
    asm!("ltr {selector:x}", selector = in(reg) TSS_SELECTOR, options(nostack, preserves_flags));
}

pub const fn double_fault_ist() -> u8 {
    DOUBLE_FAULT_IST
}
