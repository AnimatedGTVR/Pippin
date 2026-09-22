#!/usr/bin/env bash
# Build a small FAT32 SATA test disk containing the Hello app bundle.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
IMAGE="${1:-$ROOT/build/pippin-fat.img}"
mkdir -p "$(dirname "$IMAGE")"
truncate -s 64M "$IMAGE"
mkfs.vfat -F 32 -n PIPPIN "$IMAGE" >/dev/null

python3 - "$IMAGE" "$ROOT/apps/demo/hello.pipb" <<'PY'
import pathlib
import struct
import sys

image = pathlib.Path(sys.argv[1])
contents = pathlib.Path(sys.argv[2]).read_bytes()
with image.open('r+b') as disk:
    boot = disk.read(512)
    bps = struct.unpack_from('<H', boot, 11)[0]
    spc = boot[13]
    reserved = struct.unpack_from('<H', boot, 14)[0]
    fats = boot[16]
    fat_sectors = struct.unpack_from('<I', boot, 36)[0]
    root = struct.unpack_from('<I', boot, 44)[0]
    assert bps == 512 and root == 2 and len(contents) <= spc * bps
    data_start = reserved + fats * fat_sectors
    root_offset = (data_start + (root - 2) * spc) * bps
    file_offset = (data_start + spc) * bps  # cluster 3
    for fat in range(fats):
        disk.seek((reserved + fat * fat_sectors) * bps + 3 * 4)
        disk.write(struct.pack('<I', 0x0FFFFFFF))
    disk.seek(root_offset)
    directory = bytearray(disk.read(spc * bps))
    entry_at = next(i for i in range(0, len(directory), 32) if directory[i] == 0)
    directory[entry_at:entry_at + 11] = b'HELLO   PIP'
    directory[entry_at + 11] = 0x20
    struct.pack_into('<H', directory, entry_at + 26, 3)
    struct.pack_into('<I', directory, entry_at + 28, len(contents))
    disk.seek(root_offset)
    disk.write(directory)
    disk.seek(file_offset)
    disk.write(contents)
print(f'FAT32 image: {image}')
PY
