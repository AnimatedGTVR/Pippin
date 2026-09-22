#!/usr/bin/env bash
# Boot the current kernel.elf under QEMU.
#
#   scripts/run-qemu.sh            boot with serial console
#   scripts/run-qemu.sh --gdb      pause and listen on :1234 for GDB
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
ELF="${ELF:-$ROOT/build/kernel.elf}"

if [[ ! -f "$ELF" ]]; then
    echo "kernel.elf not found: $ELF (run 'make' first)" >&2
    exit 1
fi

args=(-machine q35 -m 256M -display none -serial stdio -kernel "$ELF")

case "${1:-}" in
    --gdb) args=(-s -S "${args[@]}") ;;
esac

exec qemu-system-x86_64 "${args[@]}"