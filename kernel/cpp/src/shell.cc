// Native Pippin shell definition.
//
// C++ owns the shell model and layout. Rust owns composition/input. The two
// sides meet only through the stable C ABI in <pippin/shell.h>.
#include <pippin/shell.h>
#include <pippin/control.hh>

namespace pippin::shell {

constexpr pippin_shell_item shellItem(ui::Control const& control) {
    return {
        control.text,
        control.action,
        control.kind,
        static_cast<uint8_t>(control.style),
        control.frame.x,
        control.frame.y,
        control.frame.width,
        control.frame.height,
    };
}

constexpr pippin_shell_item kPanelItems[] = {
    {"Search", "launcher.open", 'b'},
    {"Pippin", "", 'l'},
    {"WiFi  Vol  Bat", "settings.open", 'b'},
};

constexpr ui::ControlSpec kDockSpecs[] = {
    ui::fixed("Apps", "launcher.open", 'b', ui::ControlStyle::TILE, 104),
    ui::fixed("Files", "files.open", 'b', ui::ControlStyle::TILE, 104),
    ui::fixed("Settings", "settings.open", 'b', ui::ControlStyle::TILE, 104),
};

constexpr auto kDockControls = ui::flow(
    ui::Flow{
        .frame = {12, 8, 344, 52},
        .axis = ui::Axis::HORIZONTAL,
        .gap = 16,
    },
    kDockSpecs
);

constexpr pippin_shell_item kDockItems[] = {
    shellItem(kDockControls[0]),
    shellItem(kDockControls[1]),
    shellItem(kDockControls[2]),
};

constexpr pippin_shell_item kLauncherItems[] = {
    {"Applications", "", 'h'},
    {"Search apps", "launcher.search", 's'},
    {"Files", "files.open", 'b'},
    {"Settings", "settings.open", 'b'},
    {"Terminal", "terminal.open", 'b'},
};

constexpr pippin_shell_item kFilesItems[] = {
    {"Home", "", 'h'},
    {"Search files", "files.search", 's'},
    {"Documents", "files.documents.open", 'b'},
    {"Downloads", "files.downloads.open", 'b'},
};

constexpr pippin_shell_item kSettingsItems[] = {
    {"Settings", "", 'h'},
    {"Appearance", "", 'h'},
    {"Animations", "settings.animations.toggle", 't'},
    {"Desktop", "", 'h'},
    {"Show dock", "settings.dock.toggle", 't'},
};

constexpr pippin_shell_item kTerminalItems[] = {
    {"Pippin Terminal", "", 'h'},
    {"Native C++ shell online.", "", 'l'},
    {"pippin> ", "terminal.input", 's'},
};

constexpr pippin_shell_surface kSurfaces[] = {
    {
        "panel", PIPPIN_SHELL_ROLE_PANEL, 1,
        0, 0, 1024, 48,
        "Panel",
        kPanelItems,
        sizeof(kPanelItems) / sizeof(kPanelItems[0]),
    },
    {
        "dock", PIPPIN_SHELL_ROLE_DOCK, 1,
        328, 688, 368, 68,
        "Dock",
        kDockItems,
        sizeof(kDockItems) / sizeof(kDockItems[0]),
    },
    {
        "launcher", PIPPIN_SHELL_ROLE_LAUNCHER, 0,
        262, 112, 500, 400,
        "Applications",
        kLauncherItems,
        sizeof(kLauncherItems) / sizeof(kLauncherItems[0]),
    },
    {
        "files", PIPPIN_SHELL_ROLE_WINDOW, 0,
        100, 94, 620, 500,
        "Files",
        kFilesItems,
        sizeof(kFilesItems) / sizeof(kFilesItems[0]),
    },
    {
        "settings", PIPPIN_SHELL_ROLE_WINDOW, 0,
        272, 116, 520, 430,
        "Settings",
        kSettingsItems,
        sizeof(kSettingsItems) / sizeof(kSettingsItems[0]),
    },
    {
        "terminal", PIPPIN_SHELL_ROLE_WINDOW, 0,
        192, 132, 640, 420,
        "Terminal",
        kTerminalItems,
        sizeof(kTerminalItems) / sizeof(kTerminalItems[0]),
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
