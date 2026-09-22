# Control Manager

Pippin's Control Manager is the first reusable C++ UI manager for the native
desktop shell. Its first consumer is the Dock.

The design is informed by Skift's Karm UI layer, especially these ideas:

- `Node` as the basic UI object
- group/proxy composition instead of one giant widget switch
- `flow` layout for horizontal and vertical children
- fixed sizing plus grow/flexible children
- control-local input bounds rather than shell-specific hit-test constants
- style metadata carried with the control instead of inferred from its label

Karm goes much further than Pippin currently does. Karm's UI layer also has
reconciliation, reactive rebuilds, focus, drag/drop, animation, text editing,
scrolling, popovers, dialogs, and richer paint primitives. Pippin does not claim
those features yet.

## Current API

The first implementation lives in:

- `kernel/cpp/include/pippin/control.hh`
- `kernel/cpp/src/shell.cc`
- `kernel/c/include/pippin/shell.h`
- `kernel/rust/src/compositor.rs`

The C++ API is deliberately freestanding and allocation-free. A control has:

- text
- action
- kind
- style
- a local rectangle

A `ControlSpec` describes a child before layout. `ui::flow()` consumes a
`Flow` plus a fixed array of specs and returns a compile-time `ControlSet`
with final local rectangles.

Example shape:

```cpp
constexpr ui::ControlSpec specs[] = {
    ui::fixed("Apps", "launcher.open", 'b', ui::ControlStyle::TILE, 104),
    ui::fixed("Files", "files.open", 'b', ui::ControlStyle::TILE, 104),
    ui::fixed("Settings", "settings.open", 'b', ui::ControlStyle::TILE, 104),
};

constexpr auto controls = ui::flow(
    ui::Flow{
        .frame = {12, 8, 344, 52},
        .axis = ui::Axis::HORIZONTAL,
        .gap = 16,
    },
    specs
);
```

The manager also supports grow-weighted children. Remaining main-axis space is
distributed by grow weight after fixed bases and gaps are accounted for.

## ABI v2

Shell ABI v2 adds per-item layout metadata:

- style
- x/y
- width/height

A zero-sized item still means legacy row layout, which lets Pippin migrate
surfaces one at a time instead of rewriting the whole shell at once.

The Dock is the first migrated surface. Rust now renders and hit-tests Dock
controls using rectangles produced by C++. The compositor no longer hardcodes
the Apps/Files/Settings x ranges.

## Why this boundary

For now:

- C++ owns control composition and layout
- C owns the stable ABI structs and validation
- Rust owns framebuffer drawing, window ownership, and event routing

That matches Pippin's current architecture and keeps the manager useful before
the C++ shell moves fully into ring-3 ELF processes.

## Next pieces for this manager

The next Control Manager work should stay focused:

1. move the top panel to manager-provided bounds
2. add hover/pressed/disabled control state
3. add `insets`, `align`, and `grow` decorators similar in spirit to Karm
4. move generic window rows/buttons away from hardcoded 42 px spacing
5. once user apps can own surfaces directly, move this same C++ manager into the
   native app/toolkit layer
