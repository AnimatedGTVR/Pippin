//! Validate the firmware's ACPI root pointer before using any tables.

extern "C" { fn pippin_acpi_rsdp() -> *const u8; }

pub struct Root {
    pub revision: u8,
    pub xsdt_address: u64,
    pub table_count: usize,
}

fn checksum(ptr: *const u8, len: usize) -> bool {
    let mut sum = 0u8;
    for i in 0..len {
        sum = sum.wrapping_add(unsafe { ptr.add(i).read_volatile() });
    }
    sum == 0
}

pub fn discover() -> Option<Root> {
    let ptr = unsafe { pippin_acpi_rsdp() };
    if ptr.is_null() { return None; }
    if unsafe { core::slice::from_raw_parts(ptr, 8) } != b"RSD PTR "
        || !checksum(ptr, 20) { return None; }
    let revision = unsafe { ptr.add(15).read_volatile() };
    let rsdt_address = unsafe { core::ptr::read_unaligned(ptr.add(16) as *const u32) } as u64;
    let xsdt_address = if revision >= 2 {
        let len = unsafe { core::ptr::read_unaligned(ptr.add(20) as *const u32) };
        if !(36..=4096).contains(&len) || !checksum(ptr, len as usize) { return None; }
        unsafe { core::ptr::read_unaligned(ptr.add(24) as *const u64) }
    } else { 0 };
    let (address, xsdt) = if xsdt_address != 0 && xsdt_address < 0x4000_0000 {
        (xsdt_address, true)
    } else { (rsdt_address, false) };
    // The current boot map only permits direct reads below 1 GiB. Treat a
    // missing or malformed root table as failed discovery, not zero tables.
    if !(0x1000..0x4000_0000).contains(&address) { return None; }
    let table = address as *const u8;
    let signature = if xsdt { b"XSDT" } else { b"RSDT" };
    let length = unsafe { core::ptr::read_unaligned(table.add(4) as *const u32) } as usize;
    let entry_size = if xsdt { 8 } else { 4 };
    if unsafe { core::slice::from_raw_parts(table, 4) } != signature
        || !(36..=16384).contains(&length)
        || (length - 36) % entry_size != 0
        || !checksum(table, length) { return None; }
    let table_count = (length - 36) / entry_size;
    Some(Root { revision, xsdt_address, table_count })
}
