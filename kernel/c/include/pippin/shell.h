// Stable C ABI shared by the C++ shell definition and the Rust compositor.
#pragma once

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

enum {
    PIPPIN_SHELL_ABI_VERSION = 1,
    PIPPIN_SHELL_ROLE_PANEL = 'P',
    PIPPIN_SHELL_ROLE_DOCK = 'D',
    PIPPIN_SHELL_ROLE_LAUNCHER = 'L',
    PIPPIN_SHELL_ROLE_NOTIFICATION = 'N',
    PIPPIN_SHELL_ROLE_WINDOW = 'W',
};

typedef struct pippin_shell_item {
    const char* text;
    const char* action;
    uint8_t kind;
} pippin_shell_item;

typedef struct pippin_shell_surface {
    const char* id;
    uint8_t role;
    int32_t x;
    int32_t y;
    int32_t width;
    int32_t height;
    const char* title;
    const pippin_shell_item* items;
    size_t item_count;
} pippin_shell_surface;

uint32_t pippin_shell_abi_version(void);
size_t pippin_shell_surface_count(void);
const pippin_shell_surface* pippin_shell_surface_at(size_t index);

#ifdef __cplusplus
}
#endif
