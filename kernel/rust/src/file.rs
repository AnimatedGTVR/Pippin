//! Read-only File Manager for a small FAT32 app bundle and Limine boot modules.

use crate::ahci::Disk;

extern "C" { fn pippin_boot_bundle(address: *mut *const u8, size: *mut u64) -> bool; }

#[derive(Clone, Copy)]
pub struct Bundle<'a> {
    pub name: &'a str,
    pub message: &'a str,
}

static mut FAT_BUNDLE: [u8; 4096] = [0; 4096];

fn le16(bytes: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([bytes[at], bytes[at + 1]])
}
fn le32(bytes: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]])
}

fn parse(contents: &'static [u8]) -> Option<Bundle<'static>> {
    let contents = core::str::from_utf8(contents).ok()?;
    let mut lines = contents.lines();
    if lines.next()? != "PIPPIN-BUNDLE/1" { return None; }
    let name = lines.next()?.strip_prefix("name=")?;
    let message = lines.next()?.strip_prefix("message=")?;
    if name.is_empty() || name.len() > 64 || message.len() > 256 { return None; }
    Some(Bundle { name, message })
}

pub fn boot_bundle() -> Option<Bundle<'static>> {
    let (mut address, mut size) = (core::ptr::null(), 0);
    if !unsafe { pippin_boot_bundle(&mut address, &mut size) }
        || address.is_null() || size > 4096 { return None; }
    let contents = unsafe {
        core::slice::from_raw_parts(address, size as usize)
    };
    parse(contents)
}

struct Fat32 {
    reserved: u32,
    fat_size: u32,
    first_data: u32,
    sectors_per_cluster: u32,
    root: u32,
    clusters: u32,
}

impl Fat32 {
    fn mount(disk: &mut Disk) -> Option<Self> {
        let mut sector = [0u8; 512];
        if !disk.read_sector(0, &mut sector) || sector[510..] != [0x55, 0xaa]
            || le16(&sector, 11) != 512 || le16(&sector, 17) != 0 { return None; }
        let spc = sector[13] as u32;
        let reserved = le16(&sector, 14) as u32;
        let fats = sector[16] as u32;
        let fat_size = le32(&sector, 36);
        let total = le32(&sector, 32);
        let first_data = reserved.checked_add(fats.checked_mul(fat_size)?)?;
        if spc == 0 || !spc.is_power_of_two() || spc > 64 || fats == 0
            || fat_size == 0 || first_data >= total { return None; }
        let clusters = (total - first_data) / spc;
        let root = le32(&sector, 44);
        if root < 2 || root >= clusters + 2 { return None; }
        Some(Self { reserved, fat_size, first_data,
                    sectors_per_cluster: spc, root, clusters })
    }

    fn first_sector(&self, cluster: u32) -> Option<u64> {
        if cluster < 2 || cluster >= self.clusters + 2 { return None; }
        Some((self.first_data + (cluster - 2) * self.sectors_per_cluster) as u64)
    }

    fn next(&self, disk: &mut Disk, cluster: u32) -> Option<u32> {
        let byte = cluster.checked_mul(4)?;
        if byte / 512 >= self.fat_size { return None; }
        let mut sector = [0u8; 512];
        if !disk.read_sector((self.reserved + byte / 512) as u64, &mut sector) { return None; }
        let next = le32(&sector, (byte % 512) as usize) & 0x0fff_ffff;
        if next >= 0x0fff_fff8 { None } else { Some(next) }
    }
}

/// Load HELLO.PIP from the root of a FAT32 volume on the AHCI disk.
pub fn fat_bundle(disk: &mut Disk) -> Option<Bundle<'static>> {
    let fat = Fat32::mount(disk)?;
    let mut cluster = fat.root;
    let mut entry = None;
    for _ in 0..16 {
        let first = fat.first_sector(cluster)?;
        for sector_index in 0..fat.sectors_per_cluster {
            let mut sector = [0u8; 512];
            if !disk.read_sector(first + sector_index as u64, &mut sector) { return None; }
            for row in sector.chunks_exact(32) {
                if row[0] == 0 { return None; }
                if row[0] == 0xe5 || row[11] == 0x0f || row[11] & 0x18 != 0 { continue; }
                if &row[..11] == b"HELLO   PIP" {
                    let start = ((le16(row, 20) as u32) << 16) | le16(row, 26) as u32;
                    entry = Some((start, le32(row, 28) as usize));
                    break;
                }
            }
            if entry.is_some() { break; }
        }
        if entry.is_some() { break; }
        cluster = fat.next(disk, cluster)?;
    }
    let (mut cluster, size) = entry?;
    if size == 0 || size > 4096 { return None; }
    let mut copied = 0;
    while copied < size {
        let first = fat.first_sector(cluster)?;
        for sector_index in 0..fat.sectors_per_cluster {
            let mut sector = [0u8; 512];
            if !disk.read_sector(first + sector_index as u64, &mut sector) { return None; }
            let amount = core::cmp::min(512, size - copied);
            unsafe { FAT_BUNDLE[copied..copied + amount].copy_from_slice(&sector[..amount]); }
            copied += amount;
            if copied == size { break; }
        }
        if copied < size { cluster = fat.next(disk, cluster)?; }
    }
    parse(unsafe { &FAT_BUNDLE[..size] })
}
