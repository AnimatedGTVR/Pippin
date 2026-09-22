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

Karm goes much further than Pippin currently does. Pippin now has basic focus,
text editing, scrolling, retained composition, and clipped painting, but Karm
still goes far beyond it with reconciliation, reactive rebuilds, drag/drop,
animation, rich text editing, popovers, dialogs, and broader paint primitives.

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

Launcher, Files, and Settings now use retained trees. Launcher was the first
tree-based surface:

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

## Section trees

Files and Settings now use the same retained composition model instead of flat
vertical arrays.

Files is structured as:

```text
Proxy
└── Vertical Group
    ├── Home heading
    ├── Search box
    └── Horizontal Group
        ├── Documents
        └── Downloads
```

Settings uses nested vertical section groups:

```text
Proxy
└── Vertical Group
    ├── Settings heading
    ├── Appearance Group
    │   ├── Appearance heading
    │   └── Animations toggle
    └── Desktop Group
        ├── Desktop heading
        └── Show dock toggle
```

Each tree has compile-time capacity and geometry assertions, including
`TreeLayout::valid()`, so overflow or unexpected layout math fails loudly
instead of silently dropping controls.

## Content clipping and occlusion

Native window controls are clipped to the content area below the title bar.
The compositor now has clipped rectangle, border, rounded-rectangle, and text
drawing helpers. This prevents future long or scrolled layouts from painting
over window chrome or outside the window frame.

Pointer hit-testing follows the same surface boundary rule. Once the pointer is
inside the topmost visible surface, controls in lower windows are no longer
eligible. Empty space in a foreground window therefore correctly occludes
buttons behind it instead of allowing click/hover fall-through.

This is clipping groundwork, not scrolling itself yet. Scroll offsets and
runtime relayout can build on the same content rectangle later.

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

## Focus scopes and click semantics

Keyboard traversal is now scoped instead of global.

The compositor tracks either:

- the **Desktop** scope, which contains Panel and Dock controls, or
- one **Window** scope identified by the active native window ID.

Opening or clicking Launcher, Files, Settings, or Terminal selects that window
scope. `Tab` and `Shift+Tab` wrap only through focusable controls in that
scope. Clicking Panel/Dock returns traversal to the Desktop scope. If a scoped
window is minimized or removed, the compositor repairs focus to the next
topmost native window or back to Desktop.

Focused controls are validated again before Enter/Space activation, so stale
focus cannot fire an action after a surface has changed.

Pointer activation also now follows normal desktop button semantics:

1. mouse-down records the pressed control and gives it focus
2. the pressed visual remains while the button is held
3. the action fires only when the button is released over that same control
4. dragging outside before release cancels activation

Window chrome actions remain press-driven.

## Node-tree safety fixes

The retained tree now reports capacity overflow instead of silently dropping
extra leaves. `TreeLayout::valid()` can be used in `static_assert` checks;
the Launcher does this for its five-leaf tree.

`Proxy` stores a pointer to its child, so the rvalue overload is explicitly
deleted. This prevents accidentally wrapping a temporary Node and leaving a
dangling child pointer.

## Native scrolling

Native Launcher/AppWindow surfaces now have per-window vertical scroll state.
Scrolling builds directly on the content clipping boundary rather than moving
the window itself.

The compositor derives a content extent from the laid-out control rectangles
and clamps each window's `scroll_y` between zero and the maximum overflow.
Rendering subtracts that offset from managed control positions while hit-testing
adds it back, so drawing and pointer input stay in the same logical coordinate
space.

When content overflows, Pippin draws a compact scrollbar thumb at the right edge
of the content viewport.

### Input

Pippin now supports:

- mouse wheel scrolling when an IntelliMouse-compatible PS/2 device is detected
- Up / Down for line scrolling
- Page Up / Page Down for viewport-sized scrolling
- Home / End for start/end
- Tab / Shift+Tab automatically scrolling the newly focused control into view

The PS/2 driver negotiates wheel mode using the standard 200/100/80 sample-rate
sequence. Devices that do not support the fourth wheel byte remain in normal
3-byte mouse mode; keyboard scrolling still works.

Wheel input obeys topmost-surface occlusion, so scrolling over the Dock, Panel,
or another foreground surface does not leak through to a window underneath.

### Scrollable Settings test

Settings now contains retained Appearance, Desktop, Input, System, and About
sections. Its content intentionally extends beyond the default 520x430 window,
giving the scrolling path a real built-in surface to exercise.

The native shell import limit was raised from 8 to 16 controls so larger retained
trees are not truncated at the C++/Rust boundary.

## Editable text controls

The Control Manager now distinguishes two editable semantic controls:

- `ui::searchBox()` — editable search/query field with placeholder text
- `ui::textField()` — editable command/text field with a persistent prefix

Both still flatten to ordinary ABI v3 leaf controls. Their live value and caret
are compositor-side runtime state keyed by window ID + row index, so no ABI bump
was required.

Current editor behavior:

- printable ASCII insertion
- 64-character input limit
- Backspace
- Delete
- Left / Right caret movement
- Home / End caret movement while editing
- click-to-place caret
- horizontally follows the caret for long input
- live caret rendering
- Tab can focus editable controls just like buttons

Space is inserted when a text field has focus instead of triggering generic
button activation. Up/Down and PageUp/PageDown still scroll the active window
unless an editable control specifically consumes that key.

Enter submits through the existing native action path using an internal
non-printable separator between action name and value. Printable user input
cannot contain that separator. The CLI removes that wrapper before dispatch, so
it never leaks into the legacy COM2 line protocol.

Search fields retain their query after Enter. Command-style `textField`
controls clear after submission.

### Terminal

Terminal has been migrated from a flat Flow to a retained node tree and now uses
`ui::textField("pippin> ", "terminal.input")`.

The `pippin> ` prefix remains visible while the editable value is drawn after
it. Enter routes the submitted text into Pippin's existing shell command parser
and clears the field.

Command output still belongs to the bootstrap/serial shell today; a native
Terminal output/history model is the next layer rather than being faked inside
the text field itself.

### Search

Launcher and Files already used `ui::searchBox()`, so they become editable
without changing their C++ layout trees. Their current query is kept locally and
Enter produces a search submission hook. Actual result filtering is intentionally
left for the consumer layer instead of being hardcoded into the compositor.

## Why this boundary

For now:

- C++ owns control composition and layout
- C owns the stable ABI structs and validation
- Rust owns framebuffer drawing, window ownership, and event routing

That matches Pippin's current architecture and keeps the manager useful before
the C++ shell moves fully into ring-3 ELF processes.

## Next pieces

The Control Manager should stay focused:

1. add live Launcher/Files filtering consumers for search queries
2. add native Terminal output/history instead of serial-only command output
3. add draggable scrollbar thumbs and optional inertial/smooth scrolling
4. add runtime relayout so maximized/resized windows can recompute their trees
5. once user apps can own surfaces directly, move this same C++ manager into the
   native app/toolkit layer
