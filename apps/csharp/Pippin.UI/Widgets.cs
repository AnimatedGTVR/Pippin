namespace Pippin.UI;

public abstract class Widget
{
    public Rect Bounds { get; private set; }
    public virtual int PreferredHeight => 28;
    public virtual void Arrange(Rect bounds) => Bounds = bounds;
    public abstract void Paint(Canvas canvas);
    public virtual bool MouseDown(int x, int y) => false;
    public virtual bool MouseUp(int x, int y) => false;
}

public class Panel : Widget
{
    protected readonly List<Widget> children = [];
    public IReadOnlyList<Widget> Children => children;
    public void Add(Widget widget) => children.Add(widget);
    public override void Arrange(Rect bounds)
    {
        base.Arrange(bounds);
        foreach (var child in children) child.Arrange(bounds);
    }
    public override void Paint(Canvas canvas)
    {
        foreach (var child in children) child.Paint(canvas);
    }
    public override bool MouseDown(int x, int y) => children.Any(child => child.MouseDown(x, y));
    public override bool MouseUp(int x, int y) => children.Any(child => child.MouseUp(x, y));
}

public sealed class StackPanel : Panel
{
    public int Spacing { get; set; } = 12;
    public int Padding { get; set; } = 20;

    public override void Arrange(Rect bounds)
    {
        base.Arrange(bounds);
        var y = bounds.Y + Padding;
        foreach (var child in children)
        {
            child.Arrange(new Rect(bounds.X + Padding, y, Math.Max(0, bounds.Width - 2 * Padding), child.PreferredHeight));
            y += child.PreferredHeight + Spacing;
        }
    }
}

public sealed class Label(string text) : Widget
{
    public string Text { get; set; } = text;
    public uint Color { get; set; } = 0x00252b31;
    public override int PreferredHeight => 24;
    public override void Paint(Canvas canvas) => canvas.DrawText(Bounds.X, Bounds.Y + 4, Text, Color);
}

public sealed class Button(string text) : Widget
{
    private bool pressed;
    public string Text { get; set; } = text;
    public event Action? Clicked;
    public override int PreferredHeight => 40;

    public override void Paint(Canvas canvas)
    {
        // Pippin buttons use a restrained GNOME-like accent treatment with
        // lightweight Redox-style geometry rather than heavy bevels.
        var fill = pressed ? 0x002f6590u : 0x003d78a8u;
        canvas.FillRect(Bounds, fill);
        canvas.FillRect(new Rect(Bounds.X + 1, Bounds.Y + 1, Bounds.Width - 2, 1), 0x00699ac0);
        canvas.FillRect(new Rect(Bounds.X + 1, Bounds.Y + Bounds.Height - 2, Bounds.Width - 2, 1), 0x00244f70);
        canvas.DrawText(Bounds.X + 14, Bounds.Y + 13, Text, 0x00ffffff);
    }

    public override bool MouseDown(int x, int y)
    {
        if (!Bounds.Contains(x, y)) return false;
        pressed = true;
        return true;
    }

    public override bool MouseUp(int x, int y)
    {
        var wasPressed = pressed;
        pressed = false;
        if (wasPressed && Bounds.Contains(x, y)) Clicked?.Invoke();
        return wasPressed;
    }
}


public sealed class Card : Panel
{
    public int Padding { get; set; } = 14;
    public override int PreferredHeight => 72;

    public override void Arrange(Rect bounds)
    {
        base.Arrange(bounds);
        foreach (var child in children)
            child.Arrange(new Rect(bounds.X + Padding, bounds.Y + Padding,
                Math.Max(0, bounds.Width - Padding * 2), Math.Max(0, bounds.Height - Padding * 2)));
    }

    public override void Paint(Canvas canvas)
    {
        canvas.FillRect(new Rect(Bounds.X + 2, Bounds.Y + 3, Bounds.Width, Bounds.Height), 0x00d9dddf);
        canvas.FillRect(Bounds, 0x00ffffff);
        canvas.StrokeRect(Bounds, 0x00c8cdd1);
        base.Paint(canvas);
    }
}

public sealed class Separator : Widget
{
    public uint Color { get; set; } = 0x00d5d9dc;
    public override int PreferredHeight => 9;
    public override void Paint(Canvas canvas) =>
        canvas.FillRect(new Rect(Bounds.X, Bounds.Y + 4, Bounds.Width, 1), Color);
}

public sealed class CheckBox(string text, bool isChecked = false) : Widget
{
    private bool pressed;
    public string Text { get; set; } = text;
    public bool IsChecked { get; set; } = isChecked;
    public event Action<bool>? Changed;
    public override int PreferredHeight => 30;

    public override void Paint(Canvas canvas)
    {
        var box = new Rect(Bounds.X, Bounds.Y + 5, 20, 20);
        canvas.FillRect(box, IsChecked ? 0x003d78a8u : 0x00ffffffu);
        canvas.StrokeRect(box, IsChecked ? 0x002f6590u : 0x009aa2a8u);
        if (IsChecked)
        {
            canvas.FillRect(new Rect(box.X + 5, box.Y + 9, 4, 4), 0x00ffffff);
            canvas.FillRect(new Rect(box.X + 8, box.Y + 6, 7, 4), 0x00ffffff);
        }
        canvas.DrawText(Bounds.X + 30, Bounds.Y + 8, Text, 0x00252b31, 2);
    }

    public override bool MouseDown(int x, int y)
    {
        if (!Bounds.Contains(x, y)) return false;
        pressed = true;
        return true;
    }

    public override bool MouseUp(int x, int y)
    {
        var activate = pressed && Bounds.Contains(x, y);
        pressed = false;
        if (activate)
        {
            IsChecked = !IsChecked;
            Changed?.Invoke(IsChecked);
        }
        return activate;
    }
}

public sealed class Switch(string text, bool isOn = false) : Widget
{
    private bool pressed;
    public string Text { get; set; } = text;
    public bool IsOn { get; set; } = isOn;
    public event Action<bool>? Changed;
    public override int PreferredHeight => 32;

    public override void Paint(Canvas canvas)
    {
        canvas.DrawText(Bounds.X, Bounds.Y + 9, Text, 0x00252b31);
        var x = Bounds.X + Bounds.Width - 46;
        var track = new Rect(x, Bounds.Y + 5, 44, 22);
        canvas.FillRect(track, IsOn ? 0x003d78a8u : 0x00c8cdd1u);
        canvas.StrokeRect(track, IsOn ? 0x002f6590u : 0x009aa2a8u);
        var knobX = IsOn ? x + 25 : x + 3;
        canvas.FillRect(new Rect(knobX, Bounds.Y + 8, 16, 16), 0x00ffffff);
    }

    public override bool MouseDown(int x, int y)
    {
        if (!Bounds.Contains(x, y)) return false;
        pressed = true;
        return true;
    }

    public override bool MouseUp(int x, int y)
    {
        var activate = pressed && Bounds.Contains(x, y);
        pressed = false;
        if (activate)
        {
            IsOn = !IsOn;
            Changed?.Invoke(IsOn);
        }
        return activate;
    }
}


public sealed class Heading(string text) : Widget
{
    public string Text { get; set; } = text;
    public override int PreferredHeight => 32;
    public override void Paint(Canvas canvas) =>
        canvas.DrawText(Bounds.X, Bounds.Y + 6, Text, 0x001b2025, 2);
}

public sealed class TextField(string placeholder = "") : Widget
{
    private bool focused;
    public string Text { get; set; } = "";
    public string Placeholder { get; set; } = placeholder;
    public override int PreferredHeight => 38;

    public override void Paint(Canvas canvas)
    {
        canvas.FillRect(Bounds, 0x00ffffff);
        canvas.StrokeRect(Bounds, focused ? 0x003d78a8u : 0x00b7bec3u, focused ? 2 : 1);
        var shown = string.IsNullOrEmpty(Text) ? Placeholder : Text;
        canvas.DrawText(Bounds.X + 11, Bounds.Y + 12, shown,
            string.IsNullOrEmpty(Text) ? 0x00777f85u : 0x00252b31u);
    }

    public override bool MouseDown(int x, int y)
    {
        focused = Bounds.Contains(x, y);
        return focused;
    }
}

public sealed class ProgressBar : Widget
{
    private double value;
    public double Value
    {
        get => value;
        set => this.value = Math.Clamp(value, 0, 1);
    }

    public override int PreferredHeight => 18;

    public override void Paint(Canvas canvas)
    {
        canvas.FillRect(new Rect(Bounds.X, Bounds.Y + 4, Bounds.Width, 10), 0x00d5d9dc);
        canvas.FillRect(new Rect(Bounds.X, Bounds.Y + 4,
            (int)(Bounds.Width * Value), 10), 0x003d78a8);
    }
}
