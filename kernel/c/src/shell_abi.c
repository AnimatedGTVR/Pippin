// Small C validation layer for the native shell ABI.
#include <pippin/shell.h>

uint32_t pippin_shell_abi_version(void) {
    return PIPPIN_SHELL_ABI_VERSION;
}

int pippin_shell_surface_valid(const pippin_shell_surface* surface) {
    if (!surface || !surface->id || !surface->title) return 0;
    if (surface->width <= 0 || surface->height <= 0) return 0;
    if (surface->item_count != 0 && !surface->items) return 0;

    switch (surface->role) {
        case PIPPIN_SHELL_ROLE_PANEL:
        case PIPPIN_SHELL_ROLE_DOCK:
        case PIPPIN_SHELL_ROLE_LAUNCHER:
        case PIPPIN_SHELL_ROLE_NOTIFICATION:
        case PIPPIN_SHELL_ROLE_WINDOW:
            return 1;
        default:
            return 0;
    }
}
