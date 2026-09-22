//! Memory Manager (the classic Macintosh "MemMan").
//!
//! M1 responsibilities: know the physical memory layout from the bootloader
//! (Multiboot memory map) and hand out physical frames from a bitmap
//! allocator. The zone heap lives in `heap.rs`; page tables in `mm` land at
//! M1 higher-half (see `kernel/asm` + docs/kernel.md).
//!
//! Physical access convention: the low 1 GiB is (and stays) identity mapped
//! as a *direct map*, so `phys_to_virt` == identity. All pointer math in
//! this module is over direct-mapped addresses.

use core::fmt;

/// Physical address the Multiboot loader places the kernel at.
pub const KERNEL_LOAD_BASE: u64 = 0x100000;

/// Size of the identity/direct-mapped window set up by boot.S.
#[allow(dead_code)] // consumed by upcoming higher-half map
pub const BOOT_MAPPED_END: u64 = 0x4000_0000; // 1 GiB

/// Page size used by the page tables.
pub const PAGE_SIZE: u64 = 4096;

/// Huge page size used by early boot identity/direct map.
pub const LARGE_PAGE_SIZE: u64 = 2 * 1024 * 1024;

/// Target higher-half base (kernel remaps itself here at Milestone 1).
pub const HIGHER_HALF_BASE: u64 = 0xFFFF_8000_0000_0000;

/// Wavelength of a frame: 4 KiB.
pub const FRAME_SIZE: u64 = PAGE_SIZE;

/// Frames inside the initial 1 GiB identity map. The allocator must never
/// hand out physical memory that the kernel cannot yet address directly.
pub const MAX_FRAMES: usize = (BOOT_MAPPED_END / FRAME_SIZE) as usize;

const BITMAP_WORDS: usize = MAX_FRAMES / 64;

/// Multiboot *info* structure flag that marks the memory map present.
const MULTIBOOT_INFO_HAS_MMAP: u32 = 1 << 6;

// ---------------------------------------------------------------------------
// Multiboot v1 info + memory-map parsing
// ---------------------------------------------------------------------------

/// Parsed view of one firmware memory region.
#[derive(Clone, Copy)]
pub struct Region {
    pub base: u64,
    pub len: u64,
    pub kind: u32, // 1 = usable RAM, per the Multiboot/e820 spec
}

/// Iterate the Multiboot v1 memory map (`mmap_addr`/`mmap_length`, info
/// offsets 48/44). Entries are 24-byte: size(4) base(8) length(8) type(4).
pub struct MemoryMap<'a> {
    ptr: *const u8,
    remaining: usize,
    _marker: core::marker::PhantomData<&'a ()>,
}

impl<'a> MemoryMap<'a> {
    /// Build a map reader from the Multiboot info pointer.
    ///
    /// # Safety
    /// `mb_info` must be a valid Multiboot info pointer from the bootloader.
    pub unsafe fn from_multiboot(mb_info: u32) -> Option<Self> {
        if mb_info == 0 {
            return None;
        }
        let p = mb_info as *const u8;
        let flags = core::ptr::read_unaligned(p as *const u32);
        if flags & MULTIBOOT_INFO_HAS_MMAP == 0 {
            return None;
        }
        let len = core::ptr::read_unaligned(p.add(44) as *const u32) as usize;
        let addr = core::ptr::read_unaligned(p.add(48) as *const u32) as usize;
        Some(MemoryMap { ptr: addr as *const u8, remaining: len, _marker: core::marker::PhantomData })
    }
}

impl Iterator for MemoryMap<'_> {
    type Item = Region;
    fn next(&mut self) -> Option<Region> {
        if self.remaining < 4 {
            return None;
        }
        // SAFETY: reader is backed by valid bootloader memory.
        let r = unsafe {
            let size = core::ptr::read_unaligned(self.ptr as *const u32) as usize;
            if size < 20 || self.remaining < size + 4 {
                return None;
            }
            let base = core::ptr::read_unaligned(self.ptr.add(4) as *const u64);
            let len = core::ptr::read_unaligned(self.ptr.add(12) as *const u64);
            let kind = core::ptr::read_unaligned(self.ptr.add(20) as *const u32);
            self.ptr = self.ptr.add(size + 4);
            self.remaining -= size + 4;
            Region { base, len, kind }
        };
        Some(r)
    }
}

/// Read `mem_upper` (KiB of memory above 1 MiB) from the Multiboot info.
///
/// # Safety
/// `mb_info` must be a valid Multiboot info pointer or zero.
pub fn multiboot_upper_memory(mb_info: u32) -> u64 {
    if mb_info == 0 {
        return 0;
    }
    unsafe {
        let p = mb_info as *const u8;
        let upper_kib = core::ptr::read_unaligned(p.add(8) as *const u32);
        u64::from(upper_kib) * 1024
    }
}

// ---------------------------------------------------------------------------
// Frame allocator
// ---------------------------------------------------------------------------

/// Bitmap across physical frames. bit = 1 means *reserved/used*.
static mut FRAME_BITMAP: [u64; BITMAP_WORDS] = [0; BITMAP_WORDS];
static mut FRAME_TOTAL: usize = 0;
static mut FRAME_FREE: usize = 0;
static mut SEARCH_HINT: usize = 0;

/// Summary handed back from [`init`], for the boot banner.
pub struct InitReport {
    pub region_count: usize,
    pub usable_kib: u64,
    pub free_frames: usize,
    pub total_frames: usize,
}

impl fmt::Display for InitReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} regions, {} KiB usable, {} total frames, {} free",
            self.region_count, self.usable_kib, self.total_frames, self.free_frames
        )
    }
}

// End of the file-backed + BSS image, as a *link-time address*. Since the
// kernel links at HIGHER_HALF_BASE (Chunk C), __bss_end is a higher-half
// VMA; translate it to a physical address before touching frames.
extern "C" {
    static __bss_end: u8;
}

/// Kernel image end, as linked (a higher-half VMA once Chunk C is in).
pub fn kernel_bss_end_vma() -> u64 {
    unsafe { (&__bss_end as *const u8) as u64 }
}

/// Translate a higher-half link-time (VMA) address to a physical address.
#[inline]
pub fn link_to_phys(vma: u64) -> u64 {
    debug_assert!(vma >= HIGHER_HALF_BASE, "not a higher-half address: {vma:#x}");
    vma - HIGHER_HALF_BASE
}

/// Physical range occupied by the kernel image.
fn kernel_image_phys_range() -> (u64, u64) {
    (KERNEL_LOAD_BASE, link_to_phys(kernel_bss_end_vma()))
}

#[inline]
fn frame_index(base: u64) -> usize {
    (base / FRAME_SIZE) as usize
}

#[inline]
fn bit_is_used(frame: usize) -> bool {
    unsafe { FRAME_BITMAP[frame / 64] & (1u64 << (frame % 64)) != 0 }
}

#[inline]
fn bit_set(frame: usize, used: bool) {
    unsafe {
        let w = &mut FRAME_BITMAP[frame / 64];
        let m = 1u64 << (frame % 64);
        if used {
            *w |= m;
        } else {
            *w &= !m;
        }
    }
}

/// Mark `[base, base+len)` frames as used or free (clamped to MAX_FRAMES).
fn paint_range(base: u64, len: u64, used: bool) {
    // Free only whole usable frames; reserve every frame touched by a used
    // region, including partial frames at its edges.
    let end = base.saturating_add(len);
    let first = if used { base / FRAME_SIZE } else { base.div_ceil(FRAME_SIZE) };
    let last = if used { end.div_ceil(FRAME_SIZE) } else { end / FRAME_SIZE };
    let first = (first as usize).max(1).min(MAX_FRAMES); // frame 0 stays reserved
    let last = (last as usize).min(MAX_FRAMES);
    for f in first..last {
        bit_set(f, used);
    }
}

/// Initialize the frame allocator from the bootloader memory map.
///
/// # Safety
/// `mb_info` must be a valid Multiboot info pointer. Called once, before
/// any allocation, from the boot path.
pub unsafe fn init(mb_info: u32) -> InitReport {
    FRAME_BITMAP.fill(u64::MAX); // start: everything reserved
    FRAME_TOTAL = MAX_FRAMES;
    FRAME_FREE = 0;
    SEARCH_HINT = 1;

    // Memory map: usable regions become free frames.
    let mut regions = 0usize;
    let mut usable_kib = 0u64;
    if let Some(map) = MemoryMap::from_multiboot(mb_info) {
        for reg in map {
            regions += 1;
            if reg.kind == 1 {
                usable_kib += reg.len / 1024;
                paint_range(reg.base, reg.len, false);
            }
        }
    } else {
        // Fallback: mem_upper only. Upper memory from 1 MiB is usable.
        let upper = multiboot_upper_memory(mb_info);
        usable_kib = upper;
        regions = 1;
        paint_range(KERNEL_LOAD_BASE, upper, false);
    }

    // Reserve special low memory:
    paint_range(0x0, 0x1000, true); // real mode IDT/BDA
    paint_range(0x9000, 0xA000, true); // Multiboot info + E820 map
    paint_range(0xA0000, 0x100000, true); // VGA + option ROM / EBDA
    paint_range(0x100000, 0x200000, true); // kick PIT/timer regs clear too

    // Reserve the kernel image itself.
    let (kstart, kend) = kernel_image_phys_range();
    paint_range(kstart, kend - kstart, true);

    // Count what remains free.
    let mut free = 0usize;
    for f in 1..MAX_FRAMES {
        if !bit_is_used(f) {
            free += 1;
        }
    }
    FRAME_FREE = free;

    InitReport {
        region_count: regions,
        usable_kib,
        total_frames: FRAME_TOTAL,
        free_frames: free,
    }
}

extern "C" {
    fn pippin_limine_region_count() -> usize;
    fn pippin_limine_region(index: usize, base: *mut u64, len: *mut u64, kind: *mut u64);
}

/// Initialize the frame bitmap from Limine's memory map. Limine marks the
/// executable and bootloader data separately, so only USABLE regions are freed.
pub unsafe fn init_limine() -> InitReport {
    FRAME_BITMAP.fill(u64::MAX);
    FRAME_TOTAL = MAX_FRAMES;
    FRAME_FREE = 0;
    SEARCH_HINT = 1;

    let regions = pippin_limine_region_count();
    let mut usable_kib = 0;
    for i in 0..regions {
        let (mut base, mut len, mut kind) = (0, 0, 0);
        pippin_limine_region(i, &mut base, &mut len, &mut kind);
        if kind == 0 {
            usable_kib += len / 1024;
            paint_range(base, len, false);
        }
    }
    let mut free = 0;
    for f in 1..MAX_FRAMES {
        if !bit_is_used(f) {
            free += 1;
        }
    }
    FRAME_FREE = free;
    InitReport { region_count: regions, usable_kib, total_frames: FRAME_TOTAL, free_frames: free }
}

/// Allocate `count` contiguous frames. Returns physical address.
pub fn alloc_frames(count: usize) -> Option<u64> {
    let mut run = 0usize;
    let mut start = 1usize;
    // SAFETY: statics used single-threaded during boot, before the scheduler.
    let mut frame = unsafe { SEARCH_HINT.max(1) };
    let mut scanned = 0usize;
    while scanned < MAX_FRAMES + count {
        if frame >= MAX_FRAMES {
            frame = 1;
            run = 0;
        }
        if !bit_is_used(frame) {
            if run == 0 {
                start = frame;
            }
            run += 1;
            if run == count {
                for f in start..start + count {
                    bit_set(f, true);
                }
                // SAFETY: statics; single-threaded boot path so far.
                unsafe { FRAME_FREE -= count; SEARCH_HINT = start + count; }
                return Some(start as u64 * FRAME_SIZE);
            }
        } else {
            run = 0;
        }
        frame += 1;
        scanned += 1;
    }
    None
}

/// Allocate one frame and zero it. Returns physical address.
pub fn alloc_zeroed_frame() -> Option<u64> {
    let phys = alloc_frames(1)?;
    // Zero via the direct map (identity for now).
    unsafe {
        core::ptr::write_bytes(phys as *mut u64, 0, (FRAME_SIZE / 8) as usize);
    }
    Some(phys)
}

/// Return a previously allocated frame (or range) to the pool.
pub fn free_frames(phys: u64, count: usize) {
    let first = frame_index(phys);
    // SAFETY: static bit state; single-threaded for now.
    unsafe {
        for f in first..first + count {
            bit_set(f, false);
        }
        FRAME_FREE += count;
    }
}

/// Number of currently free frames.
pub fn free_frame_count() -> usize {
    unsafe { FRAME_FREE }
}

#[allow(dead_code)] // consumed by upcoming higher-half map
pub fn align_up(value: u64, align: u64) -> u64 {
    debug_assert!(align.is_power_of_two());
    (value + align - 1) & !(align - 1)
}

/// Convert a physical address to its direct-map (kernel-visible) address.
/// The low 1 GiB is permanently identity mapped, so this is identity.
#[inline]
pub const fn phys_to_virt(phys: u64) -> u64 {
    phys
}
