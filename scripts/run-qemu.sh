#!/usr/bin/env bash
# Boot the current kernel.elf under QEMU.
#
#   scripts/run-qemu.sh             boot with a QEMU window and serial console
#   scripts/run-qemu.sh --headless  serial console only
#   scripts/run-qemu.sh --gdb       pause and listen on :1234 for GDB
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
ELF="${ELF:-$ROOT/build/kernel.elf}"

if [[ ! -f "$ELF" ]]; then
    echo "kernel.elf not found: $ELF (run 'make' first)" >&2
    exit 1
fi

display=gtk,show-menubar=off
gdb=false
disk=false
shell=false
window_server=false
for option in "$@"; do
    case "$option" in
        --headless) display=none ;;
        --gdb) gdb=true ;;
        --disk) disk=true ;;
        --shell) shell=true ;;
        --window-server) window_server=true ;;
        *) echo "unknown option: $option" >&2; exit 2 ;;
    esac
done

args=(-machine q35 -m 256M -display "$display" -vga std -serial stdio -kernel "$ELF")
if $shell || $window_server; then
        UI_SOCKET="$ROOT/build/pippin-ui.sock"
        rm -f "$UI_SOCKET"
        args+=( -chardev "socket,id=pippin_ui,path=$UI_SOCKET,server=on,wait=off"
                -serial chardev:pippin_ui )
fi
if $window_server; then
        MONITOR_SOCKET="$ROOT/build/pippin-monitor.sock"
        rm -f "$MONITOR_SOCKET"
        args+=( -monitor "unix:$MONITOR_SOCKET,server,nowait" )
fi
if $shell; then
        dotnet run --project "$ROOT/apps/csharp/Pippin.Shell/Pippin.Shell.csproj" \
                --no-build -- --bridge "$UI_SOCKET" &
        SHELL_PID=$!
        cleanup() { kill "$SHELL_PID" 2>/dev/null || true; wait "$SHELL_PID" 2>/dev/null || true; rm -f "$UI_SOCKET"; }
        trap cleanup EXIT
fi
if $gdb; then args=(-s -S "${args[@]}"); fi
if $disk; then
        DISK_IMAGE="$ROOT/build/pippin-fat.img"
        [[ -f "$DISK_IMAGE" ]] || { echo "disk image missing: run 'make disk'" >&2; exit 1; }
        args+=( -device ich9-ahci,id=ahci
                -drive "if=none,id=disk,file=$DISK_IMAGE,format=raw"
                -device ide-hd,drive=disk,bus=ahci.0 )
fi

if $shell || $window_server; then qemu-system-x86_64 "${args[@]}"; else exec qemu-system-x86_64 "${args[@]}"; fi
