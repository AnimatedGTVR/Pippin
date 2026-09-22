//! Minimal ELF64 application loader for native Pippin user programs.
//!
//! This bootstrap loader supports statically linked x86-64 ET_EXEC images with
//! PT_LOAD segments. It maps each segment above the kernel's low 1 GiB direct
//! map, zero-fills BSS via zeroed frames, creates a guarded user stack, and
//! starts the entry point in ring 3.

use alloc::vec::Vec;

use crate::{mem, mm, sched};

const ELF_HEADER_SIZE: usize = 64;
const PROGRAM_HEADER_SIZE: usize = 56;
const PT_LOAD: u32 = 1;
const ET_EXEC: u16 = 2;
const EM_X86_64: u16 = 62;

pub const APP_MIN: u64 = 0x0000_0001_0000_0000;
pub const APP_MAX: u64 = 0x0000_0001_0200_0000;
pub const STACK_TOP: u64 = 0x0000_0001_0400_0000;
const STACK_PAGES: usize = 4;
const MAX_PROGRAM_HEADERS: usize = 16;
const MAX_MAPPED_PAGES: usize = 256;

extern "C" {
    static pippin_hello_elf_start: u8;
    static pippin_hello_elf_end: u8;
}

#[derive(Clone, Copy, Debug)]
pub enum LoadError {
    Truncated,
    BadMagic,
    UnsupportedClass,
    UnsupportedEndian,
    UnsupportedType,
    UnsupportedMachine,
    BadHeader,
    TooManyHeaders,
    BadSegment,
    AddressOutOfRange,
    OutOfMemory,
    MapFailed,
    NoLoadSegments,
    BadEntry,
    SchedulerBusy,
}

pub struct LoadReport {
    pub entry: u64,
    pub mapped_pages: usize,
    pub image_bytes: usize,
}

#[derive(Clone, Copy)]
struct Segment {
    offset: u64,
    vaddr: u64,
    filesz: u64,
    memsz: u64,
    align: u64,
}

struct Mapping {
    virt: u64,
    phys: u64,
}

static mut ACTIVE_MAPPINGS: Option<Vec<Mapping>> = None;

fn le16(bytes: &[u8], at: usize) -> Option<u16> {
    let end = at.checked_add(2)?;
    Some(u16::from_le_bytes(bytes.get(at..end)?.try_into().ok()?))
}

fn le32(bytes: &[u8], at: usize) -> Option<u32> {
    let end = at.checked_add(4)?;
    Some(u32::from_le_bytes(bytes.get(at..end)?.try_into().ok()?))
}

fn le64(bytes: &[u8], at: usize) -> Option<u64> {
    let end = at.checked_add(8)?;
    Some(u64::from_le_bytes(bytes.get(at..end)?.try_into().ok()?))
}

fn align_down(value: u64) -> u64 {
    value & !(mem::PAGE_SIZE - 1)
}

fn align_up(value: u64) -> Option<u64> {
    value.checked_add(mem::PAGE_SIZE - 1)
        .map(|value| value & !(mem::PAGE_SIZE - 1))
}

fn mapped_phys(mappings: &[Mapping], virt_page: u64) -> Option<u64> {
    mappings.iter().find(|mapping| mapping.virt == virt_page).map(|mapping| mapping.phys)
}

fn map_page(mappings: &mut Vec<Mapping>, virt_page: u64) -> Result<u64, LoadError> {
    if let Some(phys) = mapped_phys(mappings, virt_page) {
        return Ok(phys);
    }
    if mappings.len() >= MAX_MAPPED_PAGES {
        return Err(LoadError::BadSegment);
    }
    let phys = mem::alloc_zeroed_frame().ok_or(LoadError::OutOfMemory)?;
    if mm::map_user_page(virt_page, phys).is_none() {
        mem::free_frames(phys, 1);
        return Err(LoadError::MapFailed);
    }
    mappings.push(Mapping { virt: virt_page, phys });
    Ok(phys)
}

fn rollback(mappings: &[Mapping]) {
    for mapping in mappings.iter().rev() {
        if let Some(phys) = mm::unmap_user_page(mapping.virt) {
            mem::free_frames(phys, 1);
        }
    }
}

fn parse_segments(image: &[u8]) -> Result<(u64, Vec<Segment>), LoadError> {
    if image.len() < ELF_HEADER_SIZE { return Err(LoadError::Truncated); }
    if image.get(0..4) != Some(b"\x7fELF") { return Err(LoadError::BadMagic); }
    if image[4] != 2 { return Err(LoadError::UnsupportedClass); }
    if image[5] != 1 { return Err(LoadError::UnsupportedEndian); }
    if image[6] != 1 { return Err(LoadError::BadHeader); }
    if le16(image, 16).ok_or(LoadError::Truncated)? != ET_EXEC {
        return Err(LoadError::UnsupportedType);
    }
    if le16(image, 18).ok_or(LoadError::Truncated)? != EM_X86_64 {
        return Err(LoadError::UnsupportedMachine);
    }
    if le32(image, 20).ok_or(LoadError::Truncated)? != 1 {
        return Err(LoadError::BadHeader);
    }

    let entry = le64(image, 24).ok_or(LoadError::Truncated)?;
    let phoff = le64(image, 32).ok_or(LoadError::Truncated)? as usize;
    let ehsize = le16(image, 52).ok_or(LoadError::Truncated)? as usize;
    let phentsize = le16(image, 54).ok_or(LoadError::Truncated)? as usize;
    let phnum = le16(image, 56).ok_or(LoadError::Truncated)? as usize;

    if ehsize != ELF_HEADER_SIZE || phentsize != PROGRAM_HEADER_SIZE || phnum == 0 {
        return Err(LoadError::BadHeader);
    }
    if phnum > MAX_PROGRAM_HEADERS {
        return Err(LoadError::TooManyHeaders);
    }
    let table_bytes = phnum.checked_mul(phentsize).ok_or(LoadError::BadHeader)?;
    if phoff.checked_add(table_bytes).filter(|end| *end <= image.len()).is_none() {
        return Err(LoadError::Truncated);
    }

    let mut segments = Vec::new();
    for index in 0..phnum {
        let at = phoff + index * phentsize;
        if le32(image, at).ok_or(LoadError::Truncated)? != PT_LOAD { continue; }

        let offset = le64(image, at + 8).ok_or(LoadError::Truncated)?;
        let vaddr = le64(image, at + 16).ok_or(LoadError::Truncated)?;
        let filesz = le64(image, at + 32).ok_or(LoadError::Truncated)?;
        let memsz = le64(image, at + 40).ok_or(LoadError::Truncated)?;
        let align = le64(image, at + 48).ok_or(LoadError::Truncated)?;

        if memsz == 0 { continue; }
        if filesz > memsz { return Err(LoadError::BadSegment); }
        let end = vaddr.checked_add(memsz).ok_or(LoadError::BadSegment)?;
        if vaddr < APP_MIN || end > APP_MAX || end <= vaddr {
            return Err(LoadError::AddressOutOfRange);
        }
        let file_end = offset.checked_add(filesz).ok_or(LoadError::BadSegment)?;
        if file_end > image.len() as u64 { return Err(LoadError::Truncated); }
        if align > 1 && (!align.is_power_of_two() || (vaddr.wrapping_sub(offset) & (align - 1)) != 0) {
            return Err(LoadError::BadSegment);
        }

        segments.push(Segment { offset, vaddr, filesz, memsz, align });
    }

    if segments.is_empty() { return Err(LoadError::NoLoadSegments); }
    if entry < APP_MIN || entry >= APP_MAX {
        return Err(LoadError::BadEntry);
    }
    let entry_in_segment = segments.iter().any(|segment| {
        entry >= segment.vaddr && entry < segment.vaddr.saturating_add(segment.memsz)
    });
    if !entry_in_segment { return Err(LoadError::BadEntry); }

    Ok((entry, segments))
}

fn release_finished_app() -> Result<(), LoadError> {
    unsafe {
        if ACTIVE_MAPPINGS.is_some() {
            if !sched::task_dead(3) {
                return Err(LoadError::SchedulerBusy);
            }
            if let Some(old) = ACTIVE_MAPPINGS.take() {
                rollback(&old);
            }
        }
    }
    Ok(())
}

pub fn reap_if_exited() {
    unsafe {
        if ACTIVE_MAPPINGS.is_some() && sched::task_dead(3) {
            if let Some(old) = ACTIVE_MAPPINGS.take() {
                rollback(&old);
            }
        }
    }
}

pub fn load_and_spawn(image: &[u8], event_port: u16) -> Result<LoadReport, LoadError> {
    release_finished_app()?;
    let (entry, segments) = parse_segments(image)?;
    let mut mappings = Vec::new();

    for segment in &segments {
        let first = align_down(segment.vaddr);
        let end = align_up(segment.vaddr.checked_add(segment.memsz).ok_or(LoadError::BadSegment)?)
            .ok_or(LoadError::BadSegment)?;
        let mut page = first;
        while page < end {
            if let Err(error) = map_page(&mut mappings, page) {
                rollback(&mappings);
                return Err(error);
            }
            page = page.checked_add(mem::PAGE_SIZE).ok_or(LoadError::BadSegment)?;
        }
    }

    // Copy file-backed bytes into their mapped frames. Fresh frames are
    // already zeroed, so bytes between p_filesz and p_memsz become BSS.
    for segment in &segments {
        let mut copied = 0u64;
        while copied < segment.filesz {
            let virt = segment.vaddr + copied;
            let page = align_down(virt);
            let page_offset = (virt - page) as usize;
            let phys = mapped_phys(&mappings, page).ok_or(LoadError::MapFailed)?;
            let remaining_page = mem::PAGE_SIZE as usize - page_offset;
            let remaining_file = (segment.filesz - copied) as usize;
            let amount = core::cmp::min(remaining_page, remaining_file);
            let source_at = (segment.offset + copied) as usize;
            unsafe {
                core::ptr::copy_nonoverlapping(
                    image.as_ptr().add(source_at),
                    (mem::phys_to_virt(phys) as *mut u8).add(page_offset),
                    amount,
                );
            }
            copied += amount as u64;
        }
    }

    let stack_base = STACK_TOP - STACK_PAGES as u64 * mem::PAGE_SIZE;
    for index in 0..STACK_PAGES {
        let virt = stack_base + index as u64 * mem::PAGE_SIZE;
        if let Err(error) = map_page(&mut mappings, virt) {
            rollback(&mappings);
            return Err(error);
        }
    }

    if !sched::spawn_user(entry, STACK_TOP - 8, event_port) {
        rollback(&mappings);
        return Err(LoadError::SchedulerBusy);
    }

    let mapped_pages = mappings.len();
    unsafe { ACTIVE_MAPPINGS = Some(mappings); }

    Ok(LoadReport {
        entry,
        mapped_pages,
        image_bytes: image.len(),
    })
}

pub fn embedded_hello() -> &'static [u8] {
    let start = unsafe { &pippin_hello_elf_start as *const u8 as usize };
    let end = unsafe { &pippin_hello_elf_end as *const u8 as usize };
    if end <= start { return &[]; }
    unsafe { core::slice::from_raw_parts(start as *const u8, end - start) }
}
