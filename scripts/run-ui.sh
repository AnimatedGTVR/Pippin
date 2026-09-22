#!/usr/bin/env bash
# Two independent C# application processes share Pippin's window server.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
GUEST_SOCKET="$ROOT/build/pippin-ui.sock"
CLIENT_SOCKET="$ROOT/build/pippin-clients.sock"
rm -f "$CLIENT_SOCKET"

dotnet run --project "$ROOT/apps/csharp/Pippin.Broker/Pippin.Broker.csproj" \
        --no-build -- "$GUEST_SOCKET" "$CLIENT_SOCKET" &
BROKER_PID=$!
dotnet run --project "$ROOT/apps/csharp/Pippin.Examples/Pippin.Examples.csproj" \
        --no-build -- about "$CLIENT_SOCKET" &
ABOUT_PID=$!
dotnet run --project "$ROOT/apps/csharp/Pippin.Examples/Pippin.Examples.csproj" \
        --no-build -- tasks "$CLIENT_SOCKET" &
TASKS_PID=$!

cleanup() {
    kill "$ABOUT_PID" "$TASKS_PID" "$BROKER_PID" 2>/dev/null || true
    wait "$ABOUT_PID" "$TASKS_PID" "$BROKER_PID" 2>/dev/null || true
    rm -f "$CLIENT_SOCKET" "$GUEST_SOCKET" "$ROOT/build/pippin-monitor.sock"
}
trap cleanup EXIT

read -r -a QEMU_MODE_ARGS <<< "${PIPPIN_QEMU_MODE:-}"
"$ROOT/scripts/run-qemu.sh" "${QEMU_MODE_ARGS[@]}" --window-server
