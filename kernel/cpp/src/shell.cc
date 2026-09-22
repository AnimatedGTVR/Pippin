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

constexpr ui::Flow windowContent(int32_t width, int32_t height, int32_t gap = 10) {
    return ui::inset(
        ui::Flow{
            .frame = {0, 44, width, height - 44},
            .axis = ui::Axis::VERTICAL,
            .gap = gap,
        },
        ui::Insets{20, 24, 20, 24}
    );
}

constexpr ui::Node kLauncherActionNodes[] = {
    ui::cross(ui::grow(ui::leaf(ui::button("Files", "files.open")), 1), 64),
    ui::cross(ui::grow(ui::leaf(ui::button("Settings", "settings.open")), 1), 64),
    ui::cross(ui::grow(ui::leaf(ui::button("Terminal", "terminal.open")), 1), 64),
};

constexpr ui::Node kLauncherActionRow =
    ui::fixed(ui::group(ui::Axis::HORIZONTAL, kLauncherActionNodes, 12), 90);

constexpr ui::Node kLauncherColumnNodes[] = {
    ui::leaf(ui::heading("Applications")),
    ui::leaf(ui::searchBox("Search apps", "launcher.search")),
    kLauncherActionRow,
};

constexpr ui::Node kLauncherColumn =
    ui::group(ui::Axis::VERTICAL, kLauncherColumnNodes, 12);

constexpr ui::Node kLauncherTree =
    ui::proxy(kLauncherColumn, ui::Insets{20, 24, 20, 24});

constexpr auto kLauncherControls = ui::layoutTree<5>(
    kLauncherTree,
    ui::Rect{0, 44, 500, 356}
);

static_assert(kLauncherControls.valid());
static_assert(kLauncherControls.size() == 5);
static_assert(kLauncherControls[0].frame.y == 64);
static_assert(kLauncherControls[1].frame.y == 106);
static_assert(kLauncherControls[2].frame.y == 169);
static_assert(kLauncherControls[2].frame.height == 64);
static_assert(kLauncherControls[2].frame.width == 142);
static_assert(kLauncherControls[4].frame.x == 333);
static_assert(kLauncherControls[4].frame.width == 143);

constexpr pippin_shell_item kLauncherItems[] = {
    shellItem(kLauncherControls[0]),
    shellItem(kLauncherControls[1]),
    shellItem(kLauncherControls[2]),
    shellItem(kLauncherControls[3]),
    shellItem(kLauncherControls[4]),
};

constexpr ui::ControlSpec kFilesSpecs[] = {
    ui::heading("Home"),
    ui::searchBox("Search files", "files.search"),
    ui::button("Documents", "files.documents.open"),
    ui::button("Downloads", "files.downloads.open"),
};

constexpr auto kFilesControls = ui::flow(
    windowContent(620, 500),
    kFilesSpecs
);

constexpr pippin_shell_item kFilesItems[] = {
    shellItem(kFilesControls[0]),
    shellItem(kFilesControls[1]),
    shellItem(kFilesControls[2]),
    shellItem(kFilesControls[3]),
};

constexpr ui::ControlSpec kSettingsSpecs[] = {
    ui::heading("Settings"),
    ui::heading("Appearance", 26),
    ui::toggle("Animations", "settings.animations.toggle"),
    ui::heading("Desktop", 26),
    ui::toggle("Show dock", "settings.dock.toggle"),
};

constexpr auto kSettingsControls = ui::flow(
    windowContent(520, 430),
    kSettingsSpecs
);

constexpr pippin_shell_item kSettingsItems[] = {
    shellItem(kSettingsControls[0]),
    shellItem(kSettingsControls[1]),
    shellItem(kSettingsControls[2]),
    shellItem(kSettingsControls[3]),
    shellItem(kSettingsControls[4]),
};

constexpr ui::ControlSpec kTerminalSpecs[] = {
    ui::heading("Pippin Terminal"),
    ui::label("Native C++ shell online."),
    ui::searchBox("pippin> ", "terminal.input"),
};

constexpr auto kTerminalControls = ui::flow(
    windowContent(640, 420),
    kTerminalSpecs
);

constexpr pippin_shell_item kTerminalItems[] = {
    shellItem(kTerminalControls[0]),
    shellItem(kTerminalControls[1]),
    shellItem(kTerminalControls[2]),
};

static_assert(kFilesControls[1].frame.width == 572);
static_assert(kSettingsControls[2].frame.width == 472);
static_assert(kTerminalControls[2].frame.width == 592);

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
