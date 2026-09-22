# Control Manager

Pippin's Control Manager is the first reusable C++ UI manager for the native
desktop shell. Its first consumers are the Dock and top Panel.

The design is informed by Skift's Karm UI layer, especially these ideas:

- `Node` as the basic UI object
- group/proxy composition instead of one giant widget switch
- `flow` layout for horizontal and vertical children
- fixed sizing plus grow/flexible children
- insets and alignment as composable layout operations
- control-local input bounds rather than shell-specific hit-test constants
- style metadata carried with the control instead of inferred from its label

Karm goes much further than Pippin currently does. Karm's UI layer also has
reconciliation, reactive rebuilds, focus, drag/drop, animation, text editing,
scrolling, popovers, dialogs, and richer paint primitives. Pippin does not claim
those features yet.

## Current API

The implementation lives in:

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

## Layout primitives

### Flow

`ui::flow()` supports horizontal and vertical layout. Fixed children consume
their basis first, then remaining main-axis space is distributed between grow
children by weight.

### Grow

`ui::grow()` and `ui::spacer()` represent flexible children. A grow child may
also carry a minimum basis before leftover space is distributed.

The top Panel uses two grow spacers around its center control so the Pippin
identity block remains centered even though the left and right controls have
different content.

### Insets

`ui::inset()` shrinks a `Rect` or `Flow` using top/right/bottom/left
insets. Convenience constructors support all-side and symmetric insets.

The Dock now starts with its full 368x68 surface and derives its inner
344x52 control region through `Insets::symmetric(8, 12)` instead of embedding
that offset directly in its flow rectangle.

### Align

`ui::cross()` constrains a control on the cross axis and applies
`START`, `CENTER`, `END`, or `FILL` alignment.

The top Panel flows across the full 48 px bar while its visible controls request
34 px height with centered cross-axis alignment, producing the 7 px vertical
padding automatically.

## Example

```cpp
constexpr ui::ControlSpec specs[] = {
    ui::cross(
        ui::fixed("Search", "launcher.open", 'b', ui::ControlStyle::SEARCH, 220),
        34
    ),
    ui::spacer(),
    ui::cross(
        ui::fixed("Pippin", "", 'l', ui::ControlStyle::SUBTLE, 184),
        34
    ),
    ui::spacer(),
    ui::cross(
        ui::fixed("WiFi  Vol  Bat", "settings.open", 'b', ui::ControlStyle::STATUS, 220),
        34
    ),
};

constexpr auto controls = ui::flow(
    ui::inset(
        ui::Flow{
            .frame = {0, 0, 1024, 48},
            .axis = ui::Axis::HORIZONTAL,
            .gap = 0,
        },
        ui::Insets::symmetric(0, 12)
    ),
    specs
);
```

## ABI v3

Shell ABI v3 carries per-item:

- style
- interaction flags
- x/y
- width/height

A zero-sized item still means legacy row layout, which lets Pippin migrate
surfaces one at a time.

Both the Dock and Panel now render and hit-test controls using rectangles
calculated by C++. The compositor no longer owns their item placement.

## Interaction state

Control Manager controls now distinguish static capability from runtime state.

C++ declares two interaction flags:

- `CONTROL_FOCUSABLE` — the control participates in keyboard traversal
- `CONTROL_DISABLED` — the control cannot focus or activate

Controls with non-empty actions become focusable by default. `ui::disabled()`
removes focusability and marks the control disabled. `ui::focusable()` can
explicitly opt a control in or out.

Rust owns transient runtime state because pointer and keyboard events already
terminate at the compositor:

- hovered control
- pressed control
- focused control

The renderer combines those runtime states with C++ style metadata. Hovered and
pressed controls receive distinct shell surfaces, focused controls receive the
accent border, and disabled controls are dimmed.

Mouse hit-testing only considers enabled controls with actions. Clicking a
focusable control also gives it keyboard focus.

Keyboard behavior:

- `Tab` moves to the next visible focusable native control
- `Shift+Tab` moves backward
- `Enter` or `Space` activates the focused control
- active external/client windows retain their own keyboard routing instead of
  having Tab intercepted by the shell

The Dock and Panel are the first surfaces using this interaction model.

## Why this boundary

For now:

- C++ owns control composition and layout
- C owns the stable ABI structs and validation
- Rust owns framebuffer drawing, window ownership, and event routing

That matches Pippin's current architecture and keeps the manager useful before
the C++ shell moves fully into ring-3 ELF processes.

## Next pieces

The Control Manager should stay focused:

1. move generic window rows/buttons away from hardcoded 42 px spacing
2. add reusable group/proxy node composition instead of flat control arrays
3. add focus scopes for individual windows/dialogs
4. add richer pointer semantics such as activate-on-release and drag cancellation
5. once user apps can own surfaces directly, move this same C++ manager into the
   native app/toolkit layer
