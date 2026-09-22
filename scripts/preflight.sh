#!/usr/bin/env bash
# Pippin host preflight.
# Detects the development machine's usable CPU/RAM/disk headroom and writes
# conservative limits consumed by the build and QEMU launch scripts.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
BUILD_DIR="${BUILD_DIR:-$ROOT/build}"
mkdir -p "$BUILD_DIR"

threads="$(getconf _NPROCESSORS_ONLN 2>/dev/null || nproc 2>/dev/null || echo 1)"
[[ "$threads" =~ ^[0-9]+$ ]] || threads=1
(( threads >= 1 )) || threads=1

mem_kib="$(awk '/^MemTotal:/ { print $2; exit }' /proc/meminfo 2>/dev/null || true)"
if [[ ! "$mem_kib" =~ ^[0-9]+$ ]]; then mem_kib=$((4 * 1024 * 1024)); fi
mem_mb=$((mem_kib / 1024))

disk_mb="$(df -Pm "$ROOT" 2>/dev/null | awk 'NR==2 { print $4 }')"
if [[ ! "$disk_mb" =~ ^[0-9]+$ ]]; then disk_mb=0; fi

cpu_model="$(awk -F: '/model name/ { sub(/^[ \t]+/, "", $2); print $2; exit }' /proc/cpuinfo 2>/dev/null || true)"
[[ -n "$cpu_model" ]] || cpu_model="Unknown CPU"

# Pippin's current early physical/direct map covers the low 1 GiB. Keep QEMU
# below that ceiling and leave plenty of RAM for the host compiler/.NET shell.
if (( mem_mb < 4096 )); then
    profile="constrained"
    qemu_mb=256
elif (( mem_mb < 8192 )); then
    profile="balanced"
    qemu_mb=384
elif (( mem_mb < 16384 )); then
    profile="comfortable"
    qemu_mb=512
else
    profile="high"
    qemu_mb=768
fi

# Keep one logical CPU for the desktop when possible and cap compiler fan-out
# so a large workstation does not accidentally create a huge build burst.
if (( threads > 1 )); then build_jobs=$((threads - 1)); else build_jobs=1; fi
(( build_jobs > 8 )) && build_jobs=8
if [[ "$profile" == "constrained" ]] && (( build_jobs > 2 )); then build_jobs=2; fi
if [[ "$profile" == "balanced" ]] && (( build_jobs > 4 )); then build_jobs=4; fi

cat > "$BUILD_DIR/pippin-limits.env" <<EOF
PIPPIN_RESOURCE_PROFILE=$profile
PIPPIN_HOST_THREADS=$threads
PIPPIN_HOST_MEMORY_MB=$mem_mb
PIPPIN_HOST_DISK_FREE_MB=$disk_mb
PIPPIN_QEMU_MEMORY_MB=$qemu_mb
PIPPIN_BUILD_JOBS=$build_jobs
EOF

printf '%s\n' "Pippin host preflight"
printf '  CPU:       %s\n' "$cpu_model"
printf '  Threads:   %s\n' "$threads"
printf '  RAM:       %s MiB\n' "$mem_mb"
printf '  Disk free: %s MiB\n' "$disk_mb"
printf '  Profile:   %s\n' "$profile"
printf '  QEMU RAM:  %s MiB\n' "$qemu_mb"
printf '  Build jobs:%s\n' "$build_jobs"
printf '  Limits:    %s\n' "$BUILD_DIR/pippin-limits.env"

if (( mem_mb < 2048 )); then
    printf '%s\n' "WARNING: Host has less than 2 GiB RAM; Pippin may build or run poorly." >&2
fi
if (( disk_mb > 0 && disk_mb < 1024 )); then
    printf '%s\n' "WARNING: Less than 1 GiB free on the project filesystem." >&2
fi
