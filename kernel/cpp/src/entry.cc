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
extern "C" __attribute__((weak)) bool pippin_limine_framebuffer(uint64_t*, uint64_t*,
    uint64_t*, uint64_t*, uint16_t*) { return false; }
extern "C" __attribute__((weak)) const uint8_t* pippin_acpi_rsdp() {
    const uint16_t ebda_segment = *reinterpret_cast<const volatile uint16_t*>(0x40e);
    const uintptr_t ebda = uintptr_t(ebda_segment) << 4;
    const char signature[] = "RSD PTR ";
    for (int region = 0; region < 2; ++region) {
        const uintptr_t start = region == 0 ? ebda : 0xe0000;
        const uintptr_t end = region == 0 ? ebda + 1024 : 0x100000;
        for (uintptr_t address = start; address < end; address += 16) {
            const auto* bytes = reinterpret_cast<const volatile uint8_t*>(address);
            bool match = true;
            for (unsigned i = 0; i < 8; ++i) match &= bytes[i] == uint8_t(signature[i]);
            if (match) return reinterpret_cast<const uint8_t*>(address);
        }
    }
    return nullptr;
}
extern "C" __attribute__((weak)) bool pippin_boot_bundle(const uint8_t**, uint64_t*) {
    return false;
}
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
