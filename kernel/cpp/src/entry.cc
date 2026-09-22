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
#include <stddef.h>

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

// The Limine image overrides these weak hooks with real memory-map accessors.
// The Multiboot image never calls them, but the shared Rust archive references
// both boot paths in its common entry routine.
extern "C" __attribute__((weak)) size_t pippin_limine_region_count() { return 0; }
extern "C" __attribute__((weak)) void pippin_limine_region(size_t, uint64_t* base,
                                                            uint64_t* len, uint64_t* kind) {
    *base = *len = *kind = 0;
}

extern "C" __attribute__((noreturn)) void kernel_entry(uint32_t mb_info) {
    run_global_ctor_tors();
    pippin_core_main(mb_info);  // Rust core does not return.
    for (;;) {
        asm volatile("hlt");
    }
}
