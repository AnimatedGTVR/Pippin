// Pippin C++ kernel runtime — public types.
#pragma once

#include <stdint.h>

namespace pippin {
namespace kabi {

// Syscall numbers. Keep in sync with kernel/rust/src/syscall.rs.
enum Syscall : uint32_t {
    SYSCALL_EXIT = 0,
    SYSCALL_LOG = 1,
    SYSCALL_MMAP = 2,
    SYSCALL_IPC_SEND = 3,
    SYSCALL_IPC_RECV = 4,
};

// Single source of truth for the C++ side version banner.
inline constexpr const char* kKernelVersion = "0.1.0";

}  // namespace kabi
}  // namespace pippin
