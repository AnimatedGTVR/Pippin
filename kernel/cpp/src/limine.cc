// Limine 64-bit entry and memory-map bridge to the Rust kernel core.
#include <limine.h>
#include <stddef.h>
#include <stdint.h>

#define REQ_SECTION(name) __attribute__((used, section(name)))

static volatile uint64_t requests_start[] REQ_SECTION(".limine_requests_start") = LIMINE_REQUESTS_START_MARKER;
static volatile uint64_t base_revision[] REQ_SECTION(".limine_requests") = LIMINE_BASE_REVISION(0);
static volatile limine_memmap_request memmap_request REQ_SECTION(".limine_requests") = {
    .id = LIMINE_MEMMAP_REQUEST_ID, .revision = 0, .response = nullptr,
};
static volatile limine_framebuffer_request framebuffer_request REQ_SECTION(".limine_requests") = {
    .id = LIMINE_FRAMEBUFFER_REQUEST_ID, .revision = 0, .response = nullptr,
};
static volatile limine_rsdp_request rsdp_request REQ_SECTION(".limine_requests") = {
    .id = LIMINE_RSDP_REQUEST_ID, .revision = 0, .response = nullptr,
};
static volatile limine_module_request module_request REQ_SECTION(".limine_requests") = {
    .id = LIMINE_MODULE_REQUEST_ID, .revision = 0, .response = nullptr,
    .internal_module_count = 0, .internal_modules = nullptr,
};
static volatile uint64_t requests_end[] REQ_SECTION(".limine_requests_end") = LIMINE_REQUESTS_END_MARKER;

using ctor_t = void (*)();
extern ctor_t __init_array_start[];
extern ctor_t __init_array_end[];

extern "C" void pippin_core_main_limine();

extern "C" size_t pippin_limine_region_count() {
    const auto* response = memmap_request.response;
    return response ? static_cast<size_t>(response->entry_count) : 0;
}

extern "C" bool pippin_limine_framebuffer(uint64_t* address, uint64_t* width,
                                            uint64_t* height, uint64_t* pitch,
                                            uint16_t* bpp) {
    const auto* response = framebuffer_request.response;
    if (!response || response->framebuffer_count == 0) return false;
    const auto* fb = response->framebuffers[0];
    if (!fb || fb->memory_model != LIMINE_FRAMEBUFFER_RGB || fb->bpp != 32) return false;
    *address = reinterpret_cast<uint64_t>(fb->address);
    *width = fb->width;
    *height = fb->height;
    *pitch = fb->pitch;
    *bpp = fb->bpp;
    return true;
}

extern "C" const uint8_t* pippin_acpi_rsdp() {
    const auto* response = rsdp_request.response;
    return response ? reinterpret_cast<const uint8_t*>(response->address) : nullptr;
}

extern "C" bool pippin_boot_bundle(const uint8_t** address, uint64_t* size) {
    const auto* response = module_request.response;
    if (!response || response->module_count == 0) return false;
    const auto* module = response->modules[0];
    if (!module || !module->address) return false;
    *address = reinterpret_cast<const uint8_t*>(module->address);
    *size = module->size;
    return true;
}

extern "C" void pippin_limine_region(size_t index, uint64_t* base,
                                      uint64_t* len, uint64_t* kind) {
    const auto* response = memmap_request.response;
    if (!response || index >= response->entry_count) {
        *base = *len = *kind = 0;
        return;
    }
    const auto* entry = response->entries[index];
    *base = entry->base;
    *len = entry->length;
    *kind = entry->type;
}

extern "C" __attribute__((noreturn)) void limine_start() {
    if (!LIMINE_BASE_REVISION_SUPPORTED(base_revision) || !memmap_request.response) {
        for (;;) asm volatile("cli; hlt");
    }
    for (ctor_t* fn = __init_array_start; fn < __init_array_end; ++fn) {
        (*fn)();
    }
    pippin_core_main_limine();
    for (;;) asm volatile("cli; hlt");
}
