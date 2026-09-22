#!/usr/bin/env bash
# Build a BIOS/UEFI hybrid ISO around the Limine protocol kernel.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
ISO_DIR="$ROOT/build/iso-root"
LIMINE_DIR="${LIMINE_DIR:-/usr/local/share/limine}"
LIMINE_BIN="${LIMINE_BIN:-limine}"
XORRISO_BIN="${XORRISO_BIN:-xorriso}"

command -v "$LIMINE_BIN" >/dev/null || { echo "limine installer not found: $LIMINE_BIN" >&2; exit 1; }
command -v "$XORRISO_BIN" >/dev/null || { echo "xorriso not found: $XORRISO_BIN" >&2; exit 1; }
[[ -f "$ROOT/build/kernel-limine.elf" ]] || { echo "run 'make iso' to build the Limine kernel" >&2; exit 1; }

mkdir -p "$ISO_DIR/boot" "$ISO_DIR/EFI/BOOT"
cp "$ROOT/build/kernel-limine.elf" "$ISO_DIR/boot/kernel-limine.elf"
cp "$ROOT/boot/limine.conf" "$ISO_DIR/boot/limine.conf"
cp "$LIMINE_DIR/limine-bios.sys" "$ISO_DIR/boot/limine-bios.sys"
cp "$LIMINE_DIR/limine-bios-cd.bin" "$ISO_DIR/boot/limine-bios-cd.bin"
cp "$LIMINE_DIR/limine-uefi-cd.bin" "$ISO_DIR/boot/limine-uefi-cd.bin"
cp "$LIMINE_DIR/BOOTX64.EFI" "$ISO_DIR/EFI/BOOT/BOOTX64.EFI"

"$XORRISO_BIN" -as mkisofs -R -r -J \
    -b boot/limine-bios-cd.bin -no-emul-boot -boot-load-size 4 -boot-info-table \
    --efi-boot boot/limine-uefi-cd.bin -efi-boot-part --efi-boot-image \
    --protective-msdos-label "$ISO_DIR" -o "$ROOT/build/pippin.iso"
"$LIMINE_BIN" bios-install "$ROOT/build/pippin.iso"
echo "ISO: build/pippin.iso"
