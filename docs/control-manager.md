# Control Manager

Pippin's Control Manager is the first reusable C++ UI manager for the native
desktop shell. The Dock, top Panel, Launcher, Files, Settings, and Terminal now
all use it for their control geometry.

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

## Retained node tree

The manager now has a retained composition layer above flat `ControlSpec`
arrays. It uses three node roles inspired by Karm's UI structure:

- `Leaf` — one semantic control
- `Group` — owns a sequence of child nodes and lays them out horizontally or
  vertically
- `Proxy` — wraps one child and transforms the frame before forwarding layout

Pippin's implementation is intentionally static: no heap allocation, no virtual
dispatch, and no runtime reconciliation yet. Nodes point at compile-time child
arrays, and `layoutTree<N>()` recursively walks the tree into a fixed-capacity
`TreeLayout<N>`.

Node-level `fixed()`, `grow()`, and `cross()` decorators control how a
whole subtree participates in its parent layout. That means a nested horizontal
button group can itself be one fixed-height child inside a vertical column.

The tree is flattened **before** the shell ABI boundary. Rust still receives the
same ABI v3 leaf controls, so Group/Proxy internals stay a C++ toolkit detail.

The Launcher is the first tree-based surface:

```text
Proxy (content insets)
└── Vertical Group
    ├── Applications heading
    ├── Search box
    └── Horizontal Group
        ├── Files
        ├── Settings
        └── Terminal
```

The horizontal action group uses grow-weighted children, while the Proxy owns
the window content padding. This is the first Pippin surface whose visible
controls come from nested layout rather than one flat flow.

## Semantic controls

On top of raw `ControlSpec`, the manager now provides small semantic
constructors:

- `ui::heading()`
- `ui::label()`
- `ui::button()`
- `ui::searchBox()`
- `ui::toggle()`

They still produce plain allocation-free `ControlSpec` values; they simply
centralize the default kind, style, focusability, and basis for common controls.

Native windows use a shared `windowContent()` vertical Flow in `shell.cc`.
The Flow starts below the 44 px title bar, applies content insets, and stacks
semantic controls with a configurable gap.

This means Launcher, Files, Settings, and Terminal no longer rely on the
compositor's old `68 + index * 42` row placement.

## ABI v3

Shell ABI v3 carries per-item:

- style
- interaction flags
- x/y
- width/height

A zero-sized item still means legacy row layout, which lets Pippin migrate
surfaces one at a time.

All built-in native shell surfaces now render and hit-test controls using
rectangles calculated by C++. The old 42 px row math remains only as a
compatibility fallback for legacy bridge-created surfaces.

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

The Dock, Panel, Launcher, Files, Settings, and Terminal all use this
interaction model once visible.

## Why this boundary

For now:

- C++ owns control composition and layout
- C owns the stable ABI structs and validation
- Rust owns framebuffer drawing, window ownership, and event routing

That matches Pippin's current architecture and keeps the manager useful before
the C++ shell moves fully into ring-3 ELF processes.

## Next pieces

The Control Manager should stay focused:

1. migrate Settings and Files to nested groups where section structure benefits
   from it
2. add focus scopes for individual windows/dialogs
3. add richer pointer semantics such as activate-on-release and drag cancellation
4. add scrolling/content clipping for longer layouts
5. once user apps can own surfaces directly, move this same C++ manager into the
   native app/toolkit layer
