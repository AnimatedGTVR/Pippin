// Small C validation layer for the native shell ABI.
#include <pippin/shell.h>

uint32_t pippin_shell_abi_version(void) {
    return PIPPIN_SHELL_ABI_VERSION;
}

int pippin_shell_surface_valid(const pippin_shell_surface* surface) {
    if (!surface || !surface->id || !surface->title) return 0;
    if (surface->width <= 0 || surface->height <= 0) return 0;
    if (surface->item_count != 0 && !surface->items) return 0;

    for (size_t i = 0; i < surface->item_count; ++i) {
        const pippin_shell_item* item = &surface->items[i];
        if (!item->text || !item->action) return 0;

        // A zero-sized frame means "legacy row layout" and remains valid while
        // surfaces migrate to the Control Manager one at a time.
        if (item->width < 0 || item->height < 0 || item->x < 0 || item->y < 0)
            return 0;
        if (item->width != 0 && item->height != 0) {
            if (item->x + item->width > surface->width ||
                item->y + item->height > surface->height)
                return 0;
        }
    }

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
