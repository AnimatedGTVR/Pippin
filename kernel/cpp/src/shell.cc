// Native Pippin shell definition.
//
// C++ owns the shell model and layout. Rust owns composition/input. The two
// sides meet only through the stable C ABI in <pippin/shell.h>.
#include <pippin/shell.h>

namespace pippin::shell {

constexpr pippin_shell_item kPanelItems[] = {
    {"Search", "launcher.open", 'b'},
    {"Pippin", "", 'l'},
    {"WiFi  Vol  Bat", "settings.open", 'b'},
};

constexpr pippin_shell_item kDockItems[] = {
    {"Apps", "launcher.open", 'b'},
    {"Files", "files.open", 'b'},
    {"Settings", "settings.open", 'b'},
};

constexpr pippin_shell_surface kSurfaces[] = {
    {
        "panel",
        PIPPIN_SHELL_ROLE_PANEL,
        0, 0, 1024, 48,
        "Panel",
        kPanelItems,
        sizeof(kPanelItems) / sizeof(kPanelItems[0]),
    },
    {
        "dock",
        PIPPIN_SHELL_ROLE_DOCK,
        328, 688, 368, 68,
        "Dock",
        kDockItems,
        sizeof(kDockItems) / sizeof(kDockItems[0]),
    },
};

} // namespace pippin::shell

extern "C" size_t pippin_shell_surface_count(void) {
    return sizeof(pippin::shell::kSurfaces) / sizeof(pippin::shell::kSurfaces[0]);
}

extern "C" const pippin_shell_surface* pippin_shell_surface_at(size_t index) {
    if (index >= pippin_shell_surface_count()) return nullptr;
    return &pippin::shell::kSurfaces[index];
}
