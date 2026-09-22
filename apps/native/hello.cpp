#include <stdint.h>

namespace {

static inline uint64_t syscall3(uint64_t number, uint64_t a0 = 0,
                                uint64_t a1 = 0, uint64_t a2 = 0) {
    uint64_t result;
    asm volatile(
        "syscall"
        : "=a"(result)
        : "a"(number), "D"(a0), "S"(a1), "d"(a2)
        : "rcx", "r11", "memory"
    );
    return result;
}

constexpr char kMessage[] =
    "  app:              ELF64 C++ user app loaded and executed\n";

} // namespace

extern "C" __attribute__((noreturn)) void _start(uint64_t event_port) {
    // Prove both IPC and syscall-visible .rodata work from the loaded ELF.
    (void)syscall3(3, event_port, 9, 0xC0DE);
    (void)syscall3(1, reinterpret_cast<uint64_t>(kMessage), sizeof(kMessage) - 1);
    (void)syscall3(0);

    for (;;) {
        asm volatile("pause");
    }
}
