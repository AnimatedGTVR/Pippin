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

enum class Axis : uint8_t {
    HORIZONTAL,
    VERTICAL,
};

enum class ControlStyle : uint8_t {
    PLAIN = 0,
    SUBTLE = 1,
    TILE = 2,
    ACCENT = 3,
    SEARCH = 4,
    STATUS = 5,
};

struct ControlSpec {
    const char* text;
    const char* action;
    uint8_t kind;
    ControlStyle style;
    int32_t basis;
    uint8_t grow;
};

struct Control {
    const char* text;
    const char* action;
    uint8_t kind;
    ControlStyle style;
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
};

constexpr ControlSpec item(const char* text, const char* action, uint8_t kind,
                           ControlStyle style = ControlStyle::PLAIN,
                           int32_t basis = 0, uint8_t grow = 1) {
    return {text, action, kind, style, basis, grow};
}

constexpr ControlSpec fixed(const char* text, const char* action, uint8_t kind,
                            ControlStyle style, int32_t basis) {
    return item(text, action, kind, style, basis, 0);
}

constexpr ControlSpec spacer(uint8_t grow = 1) {
    return {"", "", 0, ControlStyle::PLAIN, 0, grow};
}

// A deliberately small retained-layout primitive inspired by Karm Ui::flow.
// Fixed children consume their basis first. Remaining main-axis space is
// divided among grow children by weight. The cross axis fills the flow frame.
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

        Rect frame{};
        if (layout.axis == Axis::HORIZONTAL) {
            frame = {cursor, layout.frame.y, main, layout.crossSize()};
        } else {
            frame = {layout.frame.x, cursor, layout.crossSize(), main};
        }

        out.items[i] = {
            specs[i].text,
            specs[i].action,
            specs[i].kind,
            specs[i].style,
            frame,
        };

        cursor += main + layout.gap;
    }

    return out;
}

} // namespace pippin::ui
