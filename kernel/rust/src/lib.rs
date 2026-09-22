//! Pippin kernel core (Rust).
//!
//! This is the safety-critical heart of the OS: serial console for now, and
//! later the memory manager, interrupt dispatch, scheduler, IPC and syscall
//! table. It is compiled as a `no_std` staticlib and linked into `kernel.elf`
//! by CMake alongside the C++ runtime, C glue and assembly bring-up.
//!
//! Entry chain:
//!   boot.S (_start) -> kernel_entry (kernel/cpp/src/entry.cc)
//!                    -> pippin_core_main (this crate)

#![no_std]
#![no_main]
// Single-CPU bootstrap: interrupt state and MMU globals are plain mutable
// statics until the scheduler/per-CPU areas land (Milestone 2+). Rust's
// `static mut` references are sound here because exactly one CPU exists and
// interrupts are disabled around every access.
#![allow(static_mut_refs)]

extern crate alloc;

mod cpu;
mod ffi;
mod heap;
mod idt;
mod interrupts;
mod mem;
mod mm;
mod serial;

use alloc::boxed::Box;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use core::fmt::Write;
use core::panic::PanicInfo;

/// The zone heap backs every dynamic allocation on this thread.
#[global_allocator]
static ALLOCATOR: heap::HeapAllocator = heap::HeapAllocator;

/// Kernel entry point, called once from the C++ boot runtime after
/// long-mode bring-up. `mb_info` is the Multiboot v1 info pointer passed
/// through `kernel_entry` by boot.S.
///
/// This function must not return; it drops into the idle loop.
#[no_mangle]
pub extern "C" fn pippin_core_main(mb_info: u32) -> ! {
    serial::init(0x3F8);
    let mut console: serial::Console = serial::stdout();

    let _ = writeln!(console, "Pippin kernel core [rust] booting...");
    let _ = writeln!(console, "  multiboot info:     {:#010x}", mb_info);
    let _ = writeln!(console, "  upper memory:       {} KiB", mem::multiboot_upper_memory(mb_info) / 1024);
    let _ = writeln!(console, "  C++ runtime says:   {}", ffi::cpp_version());
    let _ = writeln!(console, "  drivers registered: {}", ffi::driver_count());

    // Memory manager bootstrap (frame allocator over the bootloader's map).
    let report = unsafe { mem::init(mb_info) };
    let _ = writeln!(console, "  mem map:            {}", report);
    if let Some(phys) = mem::alloc_zeroed_frame() {
        let _ = writeln!(console, "  frame sample:       {:#x} allocated, {} frames free", phys, mem::free_frame_count());
    } else {
        let _ = writeln!(console, "  frame sample:       FAILED");
    }
    if let Some(phys) = mem::alloc_frames(8) {
        mem::free_frames(phys, 8);
        let _ = writeln!(console, "  frame range:        {:#x} alloc+free, {} frames free", phys, mem::free_frame_count());
    }

    // Higher-half kernel (Chunk C): the image links at 0xFFFF800000000000+.
    // boot.S already hopped us there over its 2 MiB map; drop in real 4 KiB
    // page tables from here.
    let _ = writeln!(console, "  mm:              higher-half bss end @ {:#x}", mem::kernel_bss_end_vma());
    if let Some(report) = mm::install_initial() {
        let _ = writeln!(console, "  mm:              4K page tables: PML4={:#x}, {} frames used", report.pml4_phys, report.page_table_frames);
        let _ = writeln!(console, "  mm:              cr3={:#x}", cpu::read_cr3());
    } else {
        let _ = writeln!(console, "  mm:              page-table install FAILED");
    }

    // Zone heap bootstrap, then prove the allocator with real workload.
    let _ = writeln!(console, "  heap:              init 4 MiB zone...");
    unsafe { ALLOCATOR.init() };

    let mut vec: Vec<u32> = Vec::new();
    for i in 0..64u32 {
        vec.push(i * i);
    }
    let sum: u32 = vec.iter().sum();
    let hello: String = format!("vec[{}] sum={}", vec.len(), sum);
    let boxed: Box<u16> = Box::new(0xCAFE);
    let _ = writeln!(console, "  heap:              {} \"{}\" box={:#x}", vec.len(), hello, *boxed);

    // Interrupt subsystem: IDT + PIC + PIT (1000 Hz), then prove it.
    let _ = writeln!(console, "  int:              building IDT/PIC/PIT...");
    unsafe { interrupts::init() };
    let _ = writeln!(console, "  int:              enabled, IF=1");

    unsafe { core::arch::asm!("int3"); }
    let _ = writeln!(console, "  int:              resumed past int3");

    let t0 = interrupts::ticks();
    interrupts::sleep_ms(250);
    let t1 = interrupts::ticks();
    let _ = writeln!(console, "  int:              ticks {t0} -> {t1} (+{} in 250 ms)", t1 - t0);

    let _ = writeln!(console, "Pippin kernel core enters idle loop.");
    loop {
        cpu::halt();
    }
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    if serial::is_ready() {
        let mut console: serial::Console = serial::stdout();
        let _ = writeln!(console, "KERNEL PANIC: {info}");
    }
    loop {
        cpu::halt();
    }
}