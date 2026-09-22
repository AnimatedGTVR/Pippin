//! Single-CPU preemptive kernel-thread scheduler and process slots.
//!
//! IRQ stubs save the interrupted GPR frame and FPU state, then use the frame
//! pointer returned by this module as the stack to restore with `iretq`.

use core::arch::asm;
use core::ptr;

use crate::{cpu, gdt, idt::IntFrame};

const MAX_TASKS: usize = 4;
const STACK_SIZE: usize = 16 * 1024;
const QUANTUM_TICKS: u8 = 10;
const READY: u8 = 1;
const RUNNING: u8 = 2;
const DEAD: u8 = 3;

#[repr(align(16))]
struct Stack([u8; STACK_SIZE]);
#[repr(align(16))]
struct FxArea([u8; 512]);

struct Task {
    stack: Stack,
    fx: FxArea,
    frame: *mut IntFrame,
    stack_top: u64,
    state: u8,
    slot: u8,
    ticks: u64,
}

impl Task {
    const fn empty() -> Self {
        Self {
            stack: Stack([0; STACK_SIZE]), fx: FxArea([0; 512]),
            frame: ptr::null_mut(), stack_top: 0, state: 0, slot: 0, ticks: 0,
        }
    }
}

#[derive(Clone, Copy)]
pub struct ProcessSlot {
    pub pid: u32,
    pub main_thread: u8,
    pub event_port: u16,
    pub zone: u8,
}

static mut TASKS: [Task; MAX_TASKS] = [const { Task::empty() }; MAX_TASKS];
static mut SLOTS: [ProcessSlot; MAX_TASKS] = [ProcessSlot {
    pid: 0, main_thread: 0, event_port: 0, zone: 0,
}; MAX_TASKS];
static mut CURRENT: usize = 0;
static mut QUEUE: [usize; MAX_TASKS] = [0; MAX_TASKS];
static mut Q_HEAD: usize = 0;
static mut Q_LEN: usize = 0;
static mut REMAINING: u8 = QUANTUM_TICKS;
static mut SWITCHES: u64 = 0;

/// Read directly by `isr.S`. Null until the scheduler is initialized.
#[no_mangle]
pub static mut pippin_current_fx_area: *mut u8 = ptr::null_mut();
#[no_mangle]
pub static mut pippin_syscall_kernel_rsp: u64 = 0;

unsafe fn enqueue(index: usize) {
    if Q_LEN < MAX_TASKS {
        QUEUE[(Q_HEAD + Q_LEN) % MAX_TASKS] = index;
        Q_LEN += 1;
    }
}

unsafe fn dequeue() -> Option<usize> {
    if Q_LEN == 0 { return None; }
    let index = QUEUE[Q_HEAD];
    Q_HEAD = (Q_HEAD + 1) % MAX_TASKS;
    Q_LEN -= 1;
    Some(index)
}

unsafe fn spawn(index: usize, entry: extern "C" fn() -> !) {
    let task = &mut TASKS[index];
    let top = ((task.stack.0.as_mut_ptr() as usize + STACK_SIZE) & !15) - 8;
    let frame = (top - core::mem::size_of::<IntFrame>()) as *mut IntFrame;
    ptr::write_bytes(frame as *mut u8, 0, core::mem::size_of::<IntFrame>());
    (*frame).rip = entry as usize as u64;
    (*frame).cs = 0x08;
    (*frame).rflags = 0x202;
    (*frame).rsp = top as u64;
    (*frame).ss = 0x10;
    task.frame = frame;
    task.stack_top = top as u64;
    task.slot = index as u8;
    task.state = READY;
    asm!("fninit", options(nomem, nostack));
    asm!("fxsave64 [{}]", in(reg) task.fx.0.as_mut_ptr(), options(nostack));
    SLOTS[index] = ProcessSlot {
        pid: (index + 1) as u32,
        main_thread: index as u8,
        event_port: 0,
        zone: index as u8,
    };
    enqueue(index);
}

/// Start the boot process slot and two testable kernel threads.
/// Must run after heap, IDT, and timer initialization.
pub unsafe fn init(first: extern "C" fn() -> !, second: extern "C" fn() -> !) {
    cpu::cli();
    TASKS[0].state = RUNNING;
    TASKS[0].slot = 0;
    SLOTS[0] = ProcessSlot { pid: 1, main_thread: 0, event_port: 0, zone: 0 };
    pippin_current_fx_area = TASKS[0].fx.0.as_mut_ptr();
    asm!("mov {}, rsp", out(reg) pippin_syscall_kernel_rsp, options(nomem, nostack));
    spawn(1, first);
    spawn(2, second);
    REMAINING = QUANTUM_TICKS;
    cpu::sti();
}

unsafe fn schedule(frame: *mut IntFrame, requeue: bool) -> *mut IntFrame {
    if Q_LEN == 0 {
        if !requeue {
            TASKS[CURRENT].state = DEAD;
            // No runnable task remains. An exiting thread must never return
            // through its interrupted frame.
            loop { asm!("hlt", options(nomem, nostack)); }
        }
        REMAINING = QUANTUM_TICKS;
        return frame;
    }
    TASKS[CURRENT].frame = frame;
    if requeue {
        TASKS[CURRENT].state = READY;
        enqueue(CURRENT);
    } else {
        TASKS[CURRENT].state = DEAD;
    }
    let next = dequeue().unwrap();
    CURRENT = next;
    TASKS[next].state = RUNNING;
    REMAINING = QUANTUM_TICKS;
    SWITCHES += 1;
    pippin_current_fx_area = TASKS[next].fx.0.as_mut_ptr();
    gdt::set_kernel_stack(TASKS[next].stack_top);
    pippin_syscall_kernel_rsp = TASKS[next].stack_top;
    TASKS[next].frame
}

/// Add one ring-3 thread to the run queue after its code and stack are mapped.
pub fn spawn_user(rip: u64, user_rsp: u64, port: u16) {
    let flags = cpu::irq_save();
    unsafe {
        let index = 3;
        let task = &mut TASKS[index];
        let top = ((task.stack.0.as_mut_ptr() as usize + STACK_SIZE) & !15) - 8;
        let frame = (top - core::mem::size_of::<IntFrame>()) as *mut IntFrame;
        ptr::write_bytes(frame as *mut u8, 0, core::mem::size_of::<IntFrame>());
        (*frame).rip = rip;
        (*frame).cs = 0x33;
        (*frame).rflags = 0x202;
        (*frame).rsp = user_rsp;
        (*frame).ss = 0x2b;
        (*frame).rdi = port as u64;
        task.frame = frame;
        task.stack_top = top as u64;
        task.slot = index as u8;
        task.state = READY;
        asm!("fninit", options(nomem, nostack));
        asm!("fxsave64 [{}]", in(reg) task.fx.0.as_mut_ptr(), options(nostack));
        SLOTS[index] = ProcessSlot {
            pid: (index + 1) as u32,
            main_thread: index as u8,
            event_port: port,
            zone: 0,
        };
        enqueue(index);
    }
    cpu::irq_restore(flags);
}

/// Called only from the timer interrupt with interrupts masked.
pub unsafe fn on_tick(frame: *mut IntFrame) -> *mut IntFrame {
    if pippin_current_fx_area.is_null() { return frame; }
    TASKS[CURRENT].ticks += 1;
    REMAINING = REMAINING.saturating_sub(1);
    if REMAINING == 0 { schedule(frame, true) } else { frame }
}

pub unsafe fn on_yield(frame: *mut IntFrame) -> *mut IntFrame {
    if pippin_current_fx_area.is_null() { frame } else { schedule(frame, true) }
}

pub unsafe fn on_exit(frame: *mut IntFrame) -> *mut IntFrame {
    schedule(frame, false)
}

pub fn yield_now() {
    unsafe { asm!("int 0x40", options(nomem, preserves_flags)); }
}

pub fn exit_current() -> ! {
    unsafe { asm!("int 0x41", options(nomem, preserves_flags)); }
    loop { unsafe { asm!("hlt", options(nomem, nostack)); } }
}

pub fn current_pid() -> u32 {
    unsafe { SLOTS[TASKS[CURRENT].slot as usize].pid }
}

pub fn current_slot() -> ProcessSlot {
    unsafe { SLOTS[TASKS[CURRENT].slot as usize] }
}

pub fn switches() -> u64 { unsafe { SWITCHES } }
pub fn task_ticks(index: usize) -> u64 { unsafe { TASKS[index].ticks } }
pub fn task_dead(index: usize) -> bool { unsafe { TASKS[index].state == DEAD } }

pub fn set_event_port(port: u16) {
    unsafe { SLOTS[TASKS[CURRENT].slot as usize].event_port = port; }
}

pub fn set_zone(zone: u8) {
    unsafe { SLOTS[TASKS[CURRENT].slot as usize].zone = zone; }
}
