using System.Text.Json;
using System.Text.Json.Serialization;

namespace Pippin.UI;

// A small declarative client API. The compositor will accept these surfaces
// through IPC once Pippin can load and run managed user processes.
public enum SurfaceRole { Wallpaper, Panel, Dock, Launcher, Notification, AppWindow }

public sealed record UiNode(
    string Kind,
    string? Text = null,
    string? Action = null,
    IReadOnlyList<UiNode>? Children = null)
{
    public static UiNode Label(string text) => new("label", text);
    public static UiNode Button(string text, string action) => new("button", text, action);
    public static UiNode Heading(string text) => new("heading", text);
    public static UiNode Separator() => new("separator");
    public static UiNode Toggle(string text, string action) => new("toggle", text, action);
    public static UiNode Column(params UiNode[] children) => new("column", Children: children);
    public static UiNode Row(params UiNode[] children) => new("row", Children: children);
}

public sealed record Surface(
    string Id,
    SurfaceRole Role,
    string Title,
    int X,
    int Y,
    int Width,
    int Height,
    UiNode? Content = null,
    string? Background = null);

public abstract class Application
{
    public abstract string Id { get; }
    public abstract IEnumerable<Surface> CreateSurfaces();

    public string Describe() => JsonSerializer.Serialize(CreateSurfaces(), new JsonSerializerOptions
    {
        WriteIndented = true,
        Converters = { new JsonStringEnumConverter(JsonNamingPolicy.CamelCase) }
    });
}
