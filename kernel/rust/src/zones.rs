//! Process-owned zones and generation-checked handles for Toolbox objects.

use crate::{cpu, sched};

const MAX_ZONES: usize = 4;
const OBJECTS_PER_ZONE: usize = 16;
pub const OBJECT_BYTES: usize = 64;

#[derive(Clone, Copy)]
struct Object {
    used: bool,
    generation: u32,
    kind: u16,
    data: [u8; OBJECT_BYTES],
}

impl Object {
    const fn empty() -> Self {
        Self { used: false, generation: 1, kind: 0, data: [0; OBJECT_BYTES] }
    }
}

#[derive(Clone, Copy)]
struct Zone {
    owner: u32,
    objects: [Object; OBJECTS_PER_ZONE],
}

impl Zone {
    const fn empty() -> Self {
        Self { owner: 0, objects: [Object::empty(); OBJECTS_PER_ZONE] }
    }
}

static mut ZONES: [Zone; MAX_ZONES] = [Zone::empty(); MAX_ZONES];

fn encode(zone: usize, object: usize, generation: u32) -> u64 {
    ((zone as u64 + 1) << 48) | ((object as u64 + 1) << 32) | generation as u64
}

fn decode(handle: u64) -> Option<(usize, usize, u32)> {
    let z = ((handle >> 48) & 0xFFFF) as usize;
    let o = ((handle >> 32) & 0xFFFF) as usize;
    if z == 0 || z > MAX_ZONES || o == 0 || o > OBJECTS_PER_ZONE { return None; }
    Some((z - 1, o - 1, handle as u32))
}

/// Give the current process a zone. IDs start at 1.
pub fn create() -> Option<u8> {
    let flags = cpu::irq_save();
    let mut result = None;
    unsafe {
        for (i, zone) in ZONES.iter_mut().enumerate() {
            if zone.owner == 0 {
                *zone = Zone::empty();
                zone.owner = sched::current_pid();
                result = Some((i + 1) as u8);
                break;
            }
        }
    }
    cpu::irq_restore(flags);
    result
}

pub fn alloc(zone_id: u8, kind: u16) -> Option<u64> {
    if zone_id == 0 || zone_id as usize > MAX_ZONES { return None; }
    let flags = cpu::irq_save();
    let zone = unsafe { &mut ZONES[zone_id as usize - 1] };
    let mut result = None;
    if zone.owner == sched::current_pid() {
        for (i, obj) in zone.objects.iter_mut().enumerate() {
            if !obj.used {
                obj.used = true;
                obj.kind = kind;
                obj.data.fill(0);
                result = Some(encode(zone_id as usize - 1, i, obj.generation));
                break;
            }
        }
    }
    cpu::irq_restore(flags);
    result
}

pub fn write(handle: u64, data: &[u8]) -> bool {
    if data.len() > OBJECT_BYTES { return false; }
    let Some((z, o, generation)) = decode(handle) else { return false; };
    let flags = cpu::irq_save();
    let zone = unsafe { &mut ZONES[z] };
    let obj = &mut zone.objects[o];
    let ok = zone.owner == sched::current_pid() && obj.used && obj.generation == generation;
    if ok {
        obj.data.fill(0);
        obj.data[..data.len()].copy_from_slice(data);
    }
    cpu::irq_restore(flags);
    ok
}

/// Return a copy of the data. A caller never sees the zone's raw pointer.
pub fn read(handle: u64) -> Option<(u16, [u8; OBJECT_BYTES])> {
    let (z, o, generation) = decode(handle)?;
    let flags = cpu::irq_save();
    let zone = unsafe { &ZONES[z] };
    let obj = &zone.objects[o];
    let result = if zone.owner == sched::current_pid() && obj.used && obj.generation == generation {
        Some((obj.kind, obj.data))
    } else { None };
    cpu::irq_restore(flags);
    result
}

pub fn free(handle: u64) -> bool {
    let Some((z, o, generation)) = decode(handle) else { return false; };
    let flags = cpu::irq_save();
    let zone = unsafe { &mut ZONES[z] };
    let obj = &mut zone.objects[o];
    let ok = zone.owner == sched::current_pid() && obj.used && obj.generation == generation;
    if ok {
        obj.used = false;
        obj.generation = obj.generation.wrapping_add(1).max(1);
    }
    cpu::irq_restore(flags);
    ok
}
