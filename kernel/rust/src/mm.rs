//! mm — initial page-table construction (4 KiB pages).
//!
//! boot.S brings the CPU up on a coarse 2 MiB map (identity + higher-half
//! alias). This module replaces that with a "real" 4 KiB hierarchy: one
//! PML4 with the low 1 GiB identity-mapped and the same 1 GiB aliased at
//! HIGHER_HALF_BASE, mirroring the boot mapping at finer granularity.
//!
//! Physical access convention: everything (page tables included) lives in
//! the low 1 GiB direct map, so `mem::phys_to_virt` (still identity) is
//! used to poke entries.

use crate::mem;

pub const PRESENT: u64 = 1 << 0;
pub const WRITABLE: u64 = 1 << 1;

const LEVEL_SHIFT: [u64; 4] = [39, 30, 21, 12];
const PD_ENTRIES: usize = 512;

/// How much physical memory the initial map covers (the boot window).
pub const INITIAL_MAP_SIZE: u64 = 0x4000_0000; // 1 GiB

/// Boot banner produced by [`install_initial`].
pub struct PmapReport {
    pub pml4_phys: u64,
    pub page_table_frames: usize,
}

fn level_index(v: u64, level: usize) -> usize {
    ((v >> LEVEL_SHIFT[level]) & 0x1FF) as usize
}

/// Build the initial 4 KiB page tables and install them.
///
/// Returns None if the frame allocator cannot supply pages.
pub fn install_initial() -> Option<PmapReport> {
    let pml4 = mem::alloc_zeroed_frame()?;
    let pdpt = mem::alloc_zeroed_frame()?;
    let pd = mem::alloc_zeroed_frame()?;

    // PD: one level-1 (2 MiB) slot each, backed by 512-entry leaf tables
    // that map PHYS_BASE + j * 4 KiB -> physical 4 KiB page.
    let pd_virt = mem::phys_to_virt(pd) as *mut u64;
    let mut leaf_tables = 0usize;
    for slot in 0..PD_ENTRIES {
        let leaf = mem::alloc_zeroed_frame()?;
        leaf_tables += 1;
        let base = (slot as u64) * mem::LARGE_PAGE_SIZE;
        let leaf_virt = mem::phys_to_virt(leaf) as *mut u64;
        for j in 0..512 {
            // SAFETY: freshly zeroed table, this is its only writer.
            unsafe {
                *leaf_virt.add(j) =
                    base + (j as u64) * mem::PAGE_SIZE | PRESENT | WRITABLE;
            }
        }
        // SAFETY: freshly zeroed PD; each slot written exactly once.
        unsafe {
            *pd_virt.add(slot) = leaf | PRESENT | WRITABLE;
        }
    }

    // Wire the hierarchy. KERNEL_VMA_BASE == 0xFFFF800000000000 sign-extends
    // bit 47, so it lands in PML4 slot 256 / PDPT slot 0.
    let pml4_v = mem::phys_to_virt(pml4) as *mut u64;
    let pdpt_v = mem::phys_to_virt(pdpt) as *mut u64;
    unsafe {
        *pdpt_v.add(0) = pd | PRESENT | WRITABLE;
        *pml4_v.add(0) = pdpt | PRESENT | WRITABLE;
        *pml4_v.add(level_index(mem::HIGHER_HALF_BASE, 0)) = pdpt | PRESENT | WRITABLE;
    }
    debug_assert_eq!(level_index(mem::HIGHER_HALF_BASE, 0), 256);
    debug_assert_eq!(level_index(mem::HIGHER_HALF_BASE, 1), 0);

    // SAFETY: tables fully built and self-mapped via the identity window.
    unsafe { crate::cpu::install_paging(pml4) };

    Some(PmapReport {
        pml4_phys: pml4,
        page_table_frames: leaf_tables + 3,
    })
}