//! Single-port AHCI polling reader for SATA disks (one sector per command).

use crate::{ffi, mem, mm};

pub struct Disk {
    port: *mut u8,
    command_list: u64,
    command_table: u64,
    buffer: u64,
}

fn read(base: *mut u8, offset: usize) -> u32 {
    unsafe { (base.add(offset) as *const u32).read_volatile() }
}
fn write(base: *mut u8, offset: usize, value: u32) {
    unsafe { (base.add(offset) as *mut u32).write_volatile(value) }
}
fn wait_clear(base: *mut u8, offset: usize, bits: u32) -> bool {
    for _ in 0..1_000_000 {
        if read(base, offset) & bits == 0 { return true; }
    }
    false
}

pub fn discover() -> Option<Disk> {
    let bar = ffi::ahci_bar();
    if bar == 0 || bar & 0xfff != 0 || bar > 0xffff_ffff { return None; }
    if bar >= mm::INITIAL_MAP_SIZE { mm::map_mmio_page(bar)?; }
    let hba = bar as *mut u8;
    write(hba, 4, read(hba, 4) | (1 << 31)); // GHC.AE
    let implemented = read(hba, 0x0c);
    for index in 0..32 {
        if implemented & (1 << index) == 0 { continue; }
        if index >= 30 && bar >= mm::INITIAL_MAP_SIZE {
            mm::map_mmio_page(bar + 4096)?;
        }
        let port = unsafe { hba.add(0x100 + 0x80 * index) };
        let ssts = read(port, 0x28);
        if ssts & 0x0f != 3 || (ssts >> 8) & 0x0f != 1
            || read(port, 0x24) != 0x0000_0101 { continue; }

        // Stop command and FIS engines before replacing their DMA addresses.
        write(port, 0x18, read(port, 0x18) & !0x11);
        if !wait_clear(port, 0x18, (1 << 15) | (1 << 14)) { continue; }
        let command_list = mem::alloc_zeroed_frame()?;
        let fis = mem::alloc_zeroed_frame()?;
        let command_table = mem::alloc_zeroed_frame()?;
        let buffer = mem::alloc_zeroed_frame()?;
        write(port, 0x00, command_list as u32);
        write(port, 0x04, 0);
        write(port, 0x08, fis as u32);
        write(port, 0x0c, 0);
        write(port, 0x10, u32::MAX); // clear pending status
        write(port, 0x30, u32::MAX); // clear SATA errors
        write(port, 0x18, read(port, 0x18) | 0x10); // FIS receive
        write(port, 0x18, read(port, 0x18) | 1); // command engine
        return Some(Disk { port, command_list, command_table, buffer });
    }
    None
}

impl Disk {
    pub fn read_sector(&mut self, lba: u64, output: &mut [u8; 512]) -> bool {
        if lba >= 1 << 48 || !wait_clear(self.port, 0x20, 0x88) { return false; }
        let header = self.command_list as *mut u32;
        let table = self.command_table as *mut u8;
        unsafe {
            core::ptr::write_bytes(table, 0, 4096);
            header.add(0).write(5 | (1 << 16)); // 20-byte CFIS, one PRDT
            header.add(1).write(0);
            header.add(2).write(self.command_table as u32);
            header.add(3).write(0);
            table.add(0).write(0x27); // Register H2D FIS
            table.add(1).write(0x80); // command
            table.add(2).write(0x25); // READ DMA EXT
            for i in 0..3 { table.add(4 + i).write((lba >> (8 * i)) as u8); }
            table.add(7).write(1 << 6); // LBA mode
            for i in 0..3 { table.add(8 + i).write((lba >> (8 * (i + 3))) as u8); }
            table.add(12).write(1); // one sector
            let prdt = table.add(128) as *mut u32;
            prdt.add(0).write(self.buffer as u32);
            prdt.add(1).write(0);
            prdt.add(2).write(0);
            prdt.add(3).write(511); // byte count minus one
        }
        write(self.port, 0x10, u32::MAX);
        write(self.port, 0x38, 1);
        for _ in 0..5_000_000 {
            if read(self.port, 0x10) & (1 << 30) != 0 { return false; }
            if read(self.port, 0x38) & 1 == 0 {
                unsafe { core::ptr::copy_nonoverlapping(self.buffer as *const u8,
                    output.as_mut_ptr(), 512); }
                return true;
            }
        }
        false
    }
}
