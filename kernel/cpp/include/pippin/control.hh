#pragma once

#include <stddef.h>
#include <stdint.h>

namespace pippin::ui {

struct Rect {
    int32_t x{};
    int32_t y{};
    int32_t width{};
    int32_t height{};

    constexpr bool contains(int32_t px, int32_t py) const {
        return px >= x && py >= y && px < x + width && py < y + height;
    }
};

struct Insets {
    int32_t top{};
    int32_t right{};
    int32_t bottom{};
    int32_t left{};

    static constexpr Insets all(int32_t value) {
        return {value, value, value, value};
    }

    static constexpr Insets symmetric(int32_t vertical, int32_t horizontal) {
        return {vertical, horizontal, vertical, horizontal};
    }
};

enum class Axis : uint8_t {
    HORIZONTAL,
    VERTICAL,
};

enum class Align : uint8_t {
    START,
    CENTER,
    END,
    FILL,
};

enum class ControlStyle : uint8_t {
    PLAIN = 0,
    SUBTLE = 1,
    TILE = 2,
    ACCENT = 3,
    SEARCH = 4,
    STATUS = 5,
};

enum ControlFlag : uint8_t {
    CONTROL_NONE = 0,
    CONTROL_FOCUSABLE = 1u << 0,
    CONTROL_DISABLED = 1u << 1,
};

struct ControlSpec {
    const char* text;
    const char* action;
    uint8_t kind;
    ControlStyle style;
    int32_t basis;
    uint8_t grow;
    uint8_t flags;
    int32_t crossBasis;
    Align crossAlign;
};

struct Control {
    const char* text;
    const char* action;
    uint8_t kind;
    ControlStyle style;
    uint8_t flags;
    Rect frame;
};

template <size_t N>
struct ControlSet {
    Control items[N]{};

    constexpr size_t size() const { return N; }
    constexpr const Control* data() const { return items; }
    constexpr const Control& operator[](size_t index) const { return items[index]; }
};

struct Flow {
    Rect frame;
    Axis axis{Axis::HORIZONTAL};
    int32_t gap{};

    constexpr int32_t mainSize() const {
        return axis == Axis::HORIZONTAL ? frame.width : frame.height;
    }

    constexpr int32_t crossSize() const {
        return axis == Axis::HORIZONTAL ? frame.height : frame.width;
    }

    constexpr int32_t crossStart() const {
        return axis == Axis::HORIZONTAL ? frame.y : frame.x;
    }
};

constexpr int32_t nonNegative(int32_t value) {
    return value < 0 ? 0 : value;
}

constexpr Rect inset(Rect rect, Insets insets) {
    return {
        rect.x + insets.left,
        rect.y + insets.top,
        nonNegative(rect.width - insets.left - insets.right),
        nonNegative(rect.height - insets.top - insets.bottom),
    };
}

constexpr Flow inset(Flow layout, Insets insets) {
    layout.frame = inset(layout.frame, insets);
    return layout;
}

constexpr uint8_t defaultFlags(const char* action) {
    return action && action[0] != '\0' ? CONTROL_FOCUSABLE : CONTROL_NONE;
}

constexpr ControlSpec item(const char* text, const char* action, uint8_t kind,
                           ControlStyle style = ControlStyle::PLAIN,
                           int32_t basis = 0, uint8_t growWeight = 1) {
    return {
        text,
        action,
        kind,
        style,
        basis,
        growWeight,
        defaultFlags(action),
        0,
        Align::FILL,
    };
}

constexpr ControlSpec fixed(const char* text, const char* action, uint8_t kind,
                            ControlStyle style, int32_t basis) {
    return item(text, action, kind, style, basis, 0);
}

// Karm's Grow node is represented here as a weighted ControlSpec. A grow
// control may also have a minimum basis before remaining space is distributed.
constexpr ControlSpec grow(const char* text, const char* action, uint8_t kind,
                           ControlStyle style = ControlStyle::PLAIN,
                           uint8_t weight = 1, int32_t basis = 0) {
    return item(text, action, kind, style, basis, weight);
}

constexpr ControlSpec spacer(uint8_t growWeight = 1) {
    return grow("", "", 0, ControlStyle::PLAIN, growWeight);
}

// Small semantic constructors. These keep shell definitions readable while
// still compiling down to the same allocation-free ControlSpec data.
constexpr ControlSpec heading(const char* text, int32_t basis = 30) {
    return fixed(text, "", 'h', ControlStyle::PLAIN, basis);
}

constexpr ControlSpec label(const char* text, int32_t basis = 28) {
    return fixed(text, "", 'l', ControlStyle::PLAIN, basis);
}

constexpr ControlSpec button(const char* text, const char* action,
                             int32_t basis = 38) {
    return fixed(text, action, 'b', ControlStyle::SUBTLE, basis);
}

constexpr ControlSpec searchBox(const char* text, const char* action,
                                int32_t basis = 38) {
    return fixed(text, action, 's', ControlStyle::SEARCH, basis);
}

constexpr ControlSpec toggle(const char* text, const char* action,
                             int32_t basis = 38) {
    return fixed(text, action, 't', ControlStyle::SUBTLE, basis);
}

constexpr ControlSpec disabled(ControlSpec spec) {
    spec.flags |= CONTROL_DISABLED;
    spec.flags &= static_cast<uint8_t>(~CONTROL_FOCUSABLE);
    return spec;
}

constexpr ControlSpec focusable(ControlSpec spec, bool enabled = true) {
    if (enabled && (spec.flags & CONTROL_DISABLED) == 0)
        spec.flags |= CONTROL_FOCUSABLE;
    else
        spec.flags &= static_cast<uint8_t>(~CONTROL_FOCUSABLE);
    return spec;
}

// Constrain a control on the flow's cross axis and align it inside the cell.
// A zero cross size means fill, matching the original Control Manager behavior.
constexpr ControlSpec cross(ControlSpec spec, int32_t size,
                            Align align = Align::CENTER) {
    spec.crossBasis = nonNegative(size);
    spec.crossAlign = align;
    return spec;
}

constexpr Rect alignCross(Flow const& layout, Rect cell,
                          int32_t requested, Align align) {
    if (requested <= 0 || align == Align::FILL)
        return cell;

    const int32_t available = layout.crossSize();
    const int32_t size = requested < available ? requested : available;
    int32_t start = layout.crossStart();

    if (align == Align::CENTER)
        start += (available - size) / 2;
    else if (align == Align::END)
        start += available - size;

    if (layout.axis == Axis::HORIZONTAL) {
        cell.y = start;
        cell.height = size;
    } else {
        cell.x = start;
        cell.width = size;
    }
    return cell;
}

// A deliberately small retained-layout primitive inspired by Karm Ui::flow.
// Fixed children consume their basis first. Remaining main-axis space is
// divided among grow children by weight. The cross axis fills by default, or
// can be constrained/aligned with cross().
template <size_t N>
constexpr ControlSet<N> flow(Flow layout, const ControlSpec (&specs)[N]) {
    ControlSet<N> out{};

    int32_t fixedTotal = 0;
    uint32_t growTotal = 0;
    for (size_t i = 0; i < N; ++i) {
        fixedTotal += specs[i].basis;
        growTotal += specs[i].grow;
    }

    const int32_t gaps = N > 1 ? layout.gap * static_cast<int32_t>(N - 1) : 0;
    int32_t remaining = layout.mainSize() - fixedTotal - gaps;
    if (remaining < 0) remaining = 0;

    int32_t cursor = layout.axis == Axis::HORIZONTAL ? layout.frame.x : layout.frame.y;
    int32_t assignedGrow = 0;
    uint32_t usedGrow = 0;

    for (size_t i = 0; i < N; ++i) {
        int32_t main = specs[i].basis;

        if (specs[i].grow != 0 && growTotal != 0) {
            usedGrow += specs[i].grow;
            const int32_t target = static_cast<int32_t>(
                (static_cast<int64_t>(remaining) * usedGrow) / growTotal);
            main += target - assignedGrow;
            assignedGrow = target;
        }

        Rect cell{};
        if (layout.axis == Axis::HORIZONTAL) {
            cell = {cursor, layout.frame.y, main, layout.crossSize()};
        } else {
            cell = {layout.frame.x, cursor, layout.crossSize(), main};
        }

        const Rect frame = alignCross(
            layout,
            cell,
            specs[i].crossBasis,
            specs[i].crossAlign
        );

        out.items[i] = {
            specs[i].text,
            specs[i].action,
            specs[i].kind,
            specs[i].style,
            specs[i].flags,
            frame,
        };

        cursor += main + layout.gap;
    }

    return out;
}

} // namespace pippin::ui
