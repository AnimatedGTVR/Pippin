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
    public int Spacing { get; set; } = 10;
    public int Padding { get; set; } = 18;

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
    public uint Color { get; set; } = 0x00e8edf3;
    public override int PreferredHeight => 24;
    public override void Paint(Canvas canvas) => canvas.DrawText(Bounds.X, Bounds.Y + 4, Text, Color);
}

public sealed class Button(string text) : Widget
{
    private bool pressed;
    public string Text { get; set; } = text;
    public event Action? Clicked;
    public override int PreferredHeight => 36;

    public override void Paint(Canvas canvas)
    {
        canvas.FillRect(Bounds, pressed ? 0x003b6677u : 0x0030495bu);
        canvas.FillRect(new Rect(Bounds.X, Bounds.Y, Bounds.Width, 1), 0x00678798);
        canvas.FillRect(new Rect(Bounds.X, Bounds.Y + Bounds.Height - 1, Bounds.Width, 1), 0x001b303e);
        canvas.DrawText(Bounds.X + 12, Bounds.Y + 11, Text, 0x00f4f8fa);
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
