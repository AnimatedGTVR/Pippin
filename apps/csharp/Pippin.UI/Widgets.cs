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
