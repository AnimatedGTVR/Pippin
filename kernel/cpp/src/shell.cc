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
        control.flags,
        control.frame.x,
        control.frame.y,
        control.frame.width,
        control.frame.height,
    };
}

constexpr pippin_shell_item legacyItem(const char* text, const char* action, uint8_t kind) {
    return {text, action, kind, PIPPIN_CONTROL_STYLE_PLAIN, 0, 0, 0, 0, 0};
}

constexpr ui::ControlSpec kPanelSpecs[] = {
    ui::cross(ui::fixed("Search", "launcher.open", 'b', ui::ControlStyle::SEARCH, 220), 34),
    ui::spacer(),
    ui::cross(ui::fixed("Pippin", "", 'l', ui::ControlStyle::SUBTLE, 184), 34),
    ui::spacer(),
    ui::cross(ui::fixed("WiFi  Vol  Bat", "settings.open", 'b', ui::ControlStyle::STATUS, 220), 34),
};

constexpr auto kPanelControls = ui::flow(
    ui::inset(
        ui::Flow{
            .frame = {0, 0, 1024, 48},
            .axis = ui::Axis::HORIZONTAL,
            .gap = 0,
        },
        ui::Insets::symmetric(0, 12)
    ),
    kPanelSpecs
);

static_assert(kPanelControls[0].frame.x == 12 && kPanelControls[0].frame.y == 7);
static_assert(kPanelControls[2].frame.x == 420 && kPanelControls[2].frame.width == 184);
static_assert(kPanelControls[4].frame.x == 792 && kPanelControls[4].frame.width == 220);

constexpr pippin_shell_item kPanelItems[] = {
    shellItem(kPanelControls[0]),
    shellItem(kPanelControls[2]),
    shellItem(kPanelControls[4]),
};

constexpr ui::ControlSpec kDockSpecs[] = {
    ui::fixed("Apps", "launcher.open", 'b', ui::ControlStyle::TILE, 104),
    ui::fixed("Files", "files.open", 'b', ui::ControlStyle::TILE, 104),
    ui::fixed("Settings", "settings.open", 'b', ui::ControlStyle::TILE, 104),
};

constexpr auto kDockControls = ui::flow(
    ui::inset(
        ui::Flow{
            .frame = {0, 0, 368, 68},
            .axis = ui::Axis::HORIZONTAL,
            .gap = 16,
        },
        ui::Insets::symmetric(8, 12)
    ),
    kDockSpecs
);

static_assert(static_cast<uint8_t>(ui::ControlStyle::TILE) == PIPPIN_CONTROL_STYLE_TILE);
static_assert(ui::CONTROL_FOCUSABLE == PIPPIN_CONTROL_FLAG_FOCUSABLE);
static_assert(ui::CONTROL_DISABLED == PIPPIN_CONTROL_FLAG_DISABLED);
static_assert(kDockControls[0].frame.x == 12 && kDockControls[0].frame.width == 104);
static_assert(kDockControls[1].frame.x == 132 && kDockControls[1].frame.width == 104);
static_assert(kDockControls[2].frame.x == 252 && kDockControls[2].frame.width == 104);

constexpr pippin_shell_item kDockItems[] = {
    shellItem(kDockControls[0]),
    shellItem(kDockControls[1]),
    shellItem(kDockControls[2]),
};

constexpr pippin_shell_item kLauncherItems[] = {
    legacyItem("Applications", "", 'h'),
    legacyItem("Search apps", "launcher.search", 's'),
    legacyItem("Files", "files.open", 'b'),
    legacyItem("Settings", "settings.open", 'b'),
    legacyItem("Terminal", "terminal.open", 'b'),
};

constexpr pippin_shell_item kFilesItems[] = {
    legacyItem("Home", "", 'h'),
    legacyItem("Search files", "files.search", 's'),
    legacyItem("Documents", "files.documents.open", 'b'),
    legacyItem("Downloads", "files.downloads.open", 'b'),
};

constexpr pippin_shell_item kSettingsItems[] = {
    legacyItem("Settings", "", 'h'),
    legacyItem("Appearance", "", 'h'),
    legacyItem("Animations", "settings.animations.toggle", 't'),
    legacyItem("Desktop", "", 'h'),
    legacyItem("Show dock", "settings.dock.toggle", 't'),
};

constexpr pippin_shell_item kTerminalItems[] = {
    legacyItem("Pippin Terminal", "", 'h'),
    legacyItem("Native C++ shell online.", "", 'l'),
    legacyItem("pippin> ", "terminal.input", 's'),
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
