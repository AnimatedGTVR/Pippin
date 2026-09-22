//! Zone heap: a first-fit free-list allocator that backs `#[global_allocator]`.
//!
//! M1 scope: a single 4 MiB static zone carved out of BSS. Pre-scheduler so
//! nothing is locked; the block list is manipulated with `unsafe` interior
//! access and is documented single-threaded until the scheduler exists.
//! No coalescing yet (M1); fragmentation is acceptable for the boot heap.

use core::alloc::{GlobalAlloc, Layout};
use core::ptr;

/// Heap region size. BSS, so it costs nothing until first touch.
const HEAP_SIZE: usize = 4 * 1024 * 1024;

/// Alignment the allocator guarantees for every allocation.
const ALIGN: usize = 16;

// Heap carried in a static array. Accessed only via the allocator functions.
#[repr(align(16))]
struct HeapArray([u8; HEAP_SIZE]);
static mut HEAP: HeapArray = HeapArray([0; HEAP_SIZE]);

static mut FREE_HEAD: *mut Block = ptr::null_mut();
static mut HEAP_READY: bool = false;

/// A free block on the list. Size covers the header *and* payload. All
/// blocks are 16-byte aligned, so payloads smile after the header.
#[repr(C)]
struct Block {
    size: usize,
    next: *mut Block,
}

impl Block {
    #[inline]
    fn payload(&self) -> *mut u8 {
        (self as *const Block as usize + core::mem::size_of::<Block>()) as *mut u8
    }
}

/// Global allocator instance (unit struct; state lives in the statics above).
pub struct HeapAllocator;

impl HeapAllocator {
    /// Bootstrap the single free block that spans the whole zone.
    ///
    /// # Safety
    /// Call once from the boot path, before any allocation.
    pub unsafe fn init(&self) {
        HEAP_READY = false;
        let head = HEAP.0.as_mut_ptr() as *mut Block;
        head.write(Block { size: HEAP_SIZE, next: ptr::null_mut() });
        FREE_HEAD = head;
        HEAP_READY = true;
    }
}

unsafe impl GlobalAlloc for HeapAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if !HEAP_READY {
            return ptr::null_mut();
        }
        let want = align_up(layout.size(), ALIGN).max(ALIGN);
        let align = layout.align();

        let mut prev: *mut Block = ptr::null_mut();
        let mut cur = FREE_HEAD;
        while !cur.is_null() {
            let size = (*cur).size;
            let payload = (*cur).payload() as usize;
            let pa = align_up(payload, align);
            let used = pa + want - payload;
            if used <= size {
                // Unlink `cur` from the free list.
                let next = (*cur).next;
                if prev.is_null() {
                    FREE_HEAD = next;
                } else {
                    (*prev).next = next;
                }
                // Split off the remainder as a fresh free block, if any.
                if size - used >= core::mem::size_of::<Block>() {
                    let tail = (pa + want) as *mut Block;
                    tail.write(Block { size: size - used, next: FREE_HEAD });
                    FREE_HEAD = tail;
                }
                (*cur).size = used;
                return pa as *mut u8;
            }
            prev = cur;
            cur = (*cur).next;
        }
        ptr::null_mut()
    }

    unsafe fn dealloc(&self, ptr: *mut u8, _layout: Layout) {
        if ptr.is_null() || !HEAP_READY {
            return;
        }
        let header = (ptr as *mut Block).sub(1);
        let size = (*header).size;
        header.write(Block { size, next: FREE_HEAD });
        FREE_HEAD = header;
    }
}

#[inline]
#[allow(dead_code)] // kept for future heap growth/alignment helpers
fn align_up(v: usize, a: usize) -> usize {
    debug_assert!(a.is_power_of_two(), "align {} must be a power of two", a);
    (v + a - 1) & !(a - 1)
}