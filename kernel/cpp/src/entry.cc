// Pippin C++ kernel runtime — boot entry and cross-language bridge.
//
// The Assembly trampoline (kernel/asm/boot.S) long-jumps here with the
// Multiboot info pointer in %rdi. We run any C++ global constructors, hand
// control to the Rust core, and never expect it to return.
//
//        boot.S (_start, long mode)
//                |
//                v
//        kernel_entry(uint32_t mb_info)      <- this TU
//                |
//                v
//        pippin_core_main(uint32_t)          <- Rust staticlib (no_std)
//                |
//                v
//        idle/panic loop

#include <pippin/kernel.hh>

using ctor_t = void (*)(void);
extern ctor_t __init_array_start[];
extern ctor_t __init_array_end[];

extern "C" {
// Defined in the Rust kernel core (kernel/rust/src/lib.rs). Never returns.
void pippin_core_main(uint32_t mb_info);

// Defined in drivers/cpp/src/pci.cc — count of registered drivers.
unsigned pippin_driver_count(void);

// This TU.
const char* pippin_cpp_version(void);
}

namespace {
void run_global_ctor_tors() {
    for (ctor_t* fn = __init_array_start; fn < __init_array_end; ++fn) {
        (*fn)();
    }
}
}  // namespace

extern "C" const char* pippin_cpp_version() {
    return pippin::kabi::kKernelVersion;
}

extern "C" __attribute__((noreturn)) void kernel_entry(uint32_t mb_info) {
    run_global_ctor_tors();
    pippin_core_main(mb_info);  // Rust core does not return.
    for (;;) {
        asm volatile("hlt");
    }
}