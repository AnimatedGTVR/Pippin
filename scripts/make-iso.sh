#!/usr/bin/env bash
# Build a bootable Pippin ISO using the Limine bootloader.
#
# Not required yet: the primary dev loop is `qemu-system-x86_64 -kernel
# build/kernel.elf` (Multiboot). Once the kernel moves to the Limine
# protocol (Milestone 1, docs/boot.md) this script becomes the release path.
#
# Prerequisites: `limine` (https://limine-bootloader.org) and `xorriso`.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
ISO_DIR="$ROOT/build/iso-root"
BOOT_DIR="$ISO_DIR/boot"
LIMINE_BIN="${LIMINE_BIN:-limine}"
XORRISO_BIN="${XORRISO_BIN:-xorriso}"

command -v "$LIMINE_BIN"   >/dev/null || { echo "limine not found" >&2; exit 1; }
command -v "$XORRISO_BIN"  >/dev/null || { echo "xorriso not found" >&2; exit 1; }
[[ -f "$ROOT/build/kernel.elf" ]] || { echo "run 'make' first" >&2; exit 1; }

mkdir -p "$BOOT_DIR" "$BOOT_DIR/limine"
cp "$ROOT/build/kernel.elf"    "$BOOT_DIR/pippin.elf"
cp "$ROOT/boot/limine.conf"    "$BOOT_DIR/limine.conf"
cp "$LIMINE_BIN.bin"        "$BOOT_DIR/limine/limine-bios.bin"     2>/dev/null || true
cp "$LIMINE_BIN-bios.sys"   "$BOOT_DIR/limine/limine-bios.sys"    2>/dev/null || true
cp "$LIMINE_BIN-bios-cd.bin" "$BOOT_DIR/limine/limine-bios-cd.bin" 2>/dev/null || true
cp "$LIMINE_BIN-uefi-cd.bin" "$BOOT_DIR/limine/limine-uefi-cd.bin" 2>/dev/null || true

# x86-64 EFI runtime is forward-compat; see docs/boot.md.
mkdir -p "$BOOT_DIR/limine/EFI/BOOT"
cp "$LIMINE_BIN-uefi/BOOTX64.EFI" "$BOOT_DIR/limine/EFI/BOOT/BOOTX64.EFI" 2>/dev/null || true

"$XORRISO_BIN" -as mkisofs -b boot/limine/limine-bios-cd.bin \
    -no-emul-boot -boot-load-size 4 -boot-info-table \
    --efi-boot boot/limine/limine-uefi-cd.bin \
    -efi-boot-part --efi-boot-image --protective-msdos-label \
    "$ISO_DIR" -o "$ROOT/build/pippin.iso"

"$LIMINE_BIN" bios-install "$ROOT/build/pippin.iso"
echo "ISO: build/pippin.iso (boot with: qemu-system-x86_64 -cdrom build/pippin.iso)"