//! Pippin kernel core (Rust).
//!
//! This is the safety-critical heart of the OS: serial console, memory
//! manager, interrupt dispatch, scheduler, IPC and syscall table. It is
//! compiled as a `no_std` staticlib and linked into `kernel.elf`
//! by CMake alongside the C++ runtime, C glue and assembly bring-up.
//!
//! Entry chain:
//!   boot.S (_start) -> kernel_entry (kernel/cpp/src/entry.cc)
//!                    -> pippin_core_main (this crate)

#![no_std]
#![no_main]
// Single-CPU bootstrap: mutable globals are accessed by the one CPU. Shared
// scheduler, heap, IPC and zone mutations mask interrupts around updates.
#![allow(static_mut_refs)]

extern crate alloc;

mod cpu;
mod apic;
mod ffi;
mod event;
mod gdt;
mod heap;
mod idt;
mod interrupts;
mod ipc;
mod mem;
mod mm;
mod serial;
mod sched;
mod syscall;
mod zones;

use alloc::boxed::Box;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use core::fmt::Write;
use core::panic::PanicInfo;
use core::sync::atomic::{AtomicU64, Ordering};
use core::sync::atomic::AtomicU16;
use core::sync::atomic::AtomicBool;

static WORK_A: AtomicU64 = AtomicU64::new(0);
static WORK_B: AtomicU64 = AtomicU64::new(0);
static MAIN_PORT: AtomicU16 = AtomicU16::new(0);
static BOOT_HANDLE: AtomicU64 = AtomicU64::new(0);
static HANDLE_DENIED: AtomicBool = AtomicBool::new(false);

#[repr(align(64))]
struct Aligned([u8; 32]);

extern "C" fn worker_a() -> ! {
    let mut posted = false;
    loop {
        let n = WORK_A.fetch_add(1, Ordering::Relaxed);
        if !posted {
            posted = event::post(MAIN_PORT.load(Ordering::Relaxed), event::APPLICATION, n);
        }
        if n % 4096 == 0 {
            sched::yield_now();
        }
    }
}

extern "C" fn worker_b() -> ! {
    loop {
        WORK_B.fetch_add(1, Ordering::Relaxed);
        let handle = BOOT_HANDLE.load(Ordering::Relaxed);
        if handle != 0 && zones::read(handle).is_none() {
            HANDLE_DENIED.store(true, Ordering::Relaxed);
        }
    }
}

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
    core_main(Some(mb_info))
}

#[no_mangle]
pub extern "C" fn pippin_core_main_limine() -> ! {
    core_main(None)
}

fn core_main(mb_info: Option<u32>) -> ! {
    serial::init(0x3F8);
    let mut console: serial::Console = serial::stdout();

    let _ = writeln!(console, "Pippin kernel core [rust] booting...");
    match mb_info {
        Some(info) => {
            let _ = writeln!(console, "  boot:              Multiboot info at {info:#010x}");
            let _ = writeln!(console, "  upper memory:      {} KiB", mem::multiboot_upper_memory(info) / 1024);
        }
        None => { let _ = writeln!(console, "  boot:              Limine protocol"); }
    }
    let _ = writeln!(console, "  C++ runtime says:   {}", ffi::cpp_version());
    let _ = writeln!(console, "  drivers registered: {}", ffi::driver_count());

    // Memory manager bootstrap (frame allocator over the bootloader's map).
    let report = unsafe { match mb_info { Some(info) => mem::init(info), None => mem::init_limine() } };
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
    if mb_info.is_some() {
        if let Some(report) = mm::install_initial() {
            let _ = writeln!(console, "  mm:              4K page tables: PML4={:#x}, {} frames used", report.pml4_phys, report.page_table_frames);
            let _ = writeln!(console, "  mm:              cr3={:#x}", cpu::read_cr3());
        } else {
            let _ = writeln!(console, "  mm:              page-table install FAILED");
        }
    } else {
        let _ = writeln!(console, "  mm:              Limine page tables: cr3={:#x}", cpu::read_cr3());
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
    let aligned = Box::new(Aligned([7; 32]));
    let address = &*aligned as *const Aligned as usize;
    let value = aligned.0[0];
    drop(aligned);
    let _ = writeln!(console, "  heap:              align64={} freed value={}", address % 64 == 0, value);

    // Install the kernel GDT/TSS before the IDT uses its double-fault IST.
    unsafe { gdt::init() };
    let _ = writeln!(console, "  gdt:              TSS loaded, double-fault IST ready");

    // Interrupt subsystem: IDT + PIC + PIT (1000 Hz), then prove it.
    let _ = writeln!(console, "  int:              building IDT/PIC/PIT...");
    unsafe { interrupts::init() };
    let _ = writeln!(console, "  int:              enabled, IF=1");

    unsafe { core::arch::asm!("int3"); }
    let _ = writeln!(console, "  int:              resumed past int3");

    if unsafe { apic::init_timer(mb_info.is_some()) } {
        let _ = writeln!(console, "  apic:             periodic timer active at 1000 Hz");
    } else {
        let _ = writeln!(console, "  apic:             unavailable; PIT timer retained");
    }

    unsafe { syscall::init() };

    unsafe { sched::init(worker_a, worker_b) };
    let port = ipc::create().expect("boot event port");
    MAIN_PORT.store(port, Ordering::Relaxed);
    sched::set_event_port(port);
    event::set_boot_port(port);
    let zone = zones::create().expect("boot object zone");
    sched::set_zone(zone);
    let handle = zones::alloc(zone, 7).expect("boot object handle");
    let _ = zones::write(handle, b"Pippin");
    BOOT_HANDLE.store(handle, Ordering::Relaxed);
    let user_ready = syscall::spawn_demo(port);
    let _ = writeln!(console, "  syscall:          ring-3 demo ready={user_ready}");
    let _ = writeln!(console, "  sched:            boot slot={} pid={}, 2 threads ready",
                     sched::current_slot().main_thread, sched::current_pid());

    let t0 = interrupts::ticks();
    interrupts::sleep_ms(250);
    let t1 = interrupts::ticks();
    let _ = writeln!(console, "  int:              ticks {t0} -> {t1} (+{} in 250 ms)", t1 - t0);
    let _ = writeln!(console, "  sched:            {} switches, worker ticks {} / {}, work {} / {}",
                     sched::switches(), sched::task_ticks(1), sched::task_ticks(2),
                     WORK_A.load(Ordering::Relaxed), WORK_B.load(Ordering::Relaxed));
    let mut events = 0;
    let mut user_event = false;
    let mut timer_events = 0;
    while let Some(event) = event::poll() {
        events += 1;
        if event.source_pid == 0 && event.kind == event::TIMER { timer_events += 1; }
        if event.source_pid == 4 && event.kind == 9 && event.value == 0xC0DE {
            user_event = true;
        }
    }
    let _ = writeln!(console, "  ipc:              port={} events={} timers={} ring-3 event={}",
                     port, events, timer_events, user_event);
    let _ = writeln!(console, "  syscall:          ring-3 task exited={}", sched::task_dead(3));
    let object = zones::read(handle).expect("owned handle");
    let _ = writeln!(console, "  zones:            zone={} kind={} data={} denied={}",
                     zone, object.0, object.1[0] as char, HANDLE_DENIED.load(Ordering::Relaxed));
    let _ = zones::free(handle);

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
