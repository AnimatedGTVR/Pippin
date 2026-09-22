using Pippin.UI;
using System.Text.Json;
using System.Text.Json.Serialization;
using System.Net.Sockets;
using System.Threading.Channels;

namespace Pippin.Shell;

// Each desktop component owns its own surface. Panel and dock are normal
// clients with reserved roles; Rust owns placement, focus, and composition.
public sealed class Wallpaper : Application
{
    public override string Id => "org.pippin.wallpaper";
    public override IEnumerable<Surface> CreateSurfaces() =>
        [new("wallpaper", SurfaceRole.Wallpaper, "Wallpaper", 0, 0, 800, 600,
            Background: "#263746")];
}

public sealed class Panel : Application
{
    public override string Id => "org.pippin.panel";
    public override IEnumerable<Surface> CreateSurfaces() =>
        [new("panel", SurfaceRole.Panel, "Panel", 10, 10, 780, 40,
            UiNode.Row(UiNode.Button("Pippin", "launcher.open"),
                UiNode.Label("Pippin Desktop"), UiNode.Button("Settings", "settings.open")))];
}

public sealed class Dock : Application
{
    public override string Id => "org.pippin.dock";
    public override IEnumerable<Surface> CreateSurfaces() =>
        [new("dock", SurfaceRole.Dock, "Dock", 202, 536, 396, 54,
            UiNode.Row(UiNode.Button("Files", "files.open"),
                UiNode.Button("Settings", "settings.open"),
                UiNode.Button("Launcher", "launcher.open")))];
}

public sealed class Launcher : Application
{
    public override string Id => "org.pippin.launcher";
    public override IEnumerable<Surface> CreateSurfaces() =>
        [new("launcher", SurfaceRole.Launcher, "Applications", 24, 62, 324, 390,
            UiNode.Column(UiNode.Heading("Applications"), UiNode.Separator(),
                UiNode.Button("Files", "files.open"),
                UiNode.Button("Settings", "settings.open")))];
}

public sealed class Settings : Application
{
    public override string Id => "org.pippin.settings";
    public override IEnumerable<Surface> CreateSurfaces() =>
        [new("settings", SurfaceRole.AppWindow, "Settings", 160, 96, 480, 360,
            UiNode.Column(UiNode.Heading("Appearance"), UiNode.Separator(),
                UiNode.Button("Change wallpaper", "wallpaper.select.gradient"),
                UiNode.Toggle("Animations", "settings.animations.toggle")))];
}

public sealed class Files : Application
{
    public override string Id => "org.pippin.files";
    public override IEnumerable<Surface> CreateSurfaces() =>
        [new("files", SurfaceRole.AppWindow, "Files", 112, 80, 540, 400,
            UiNode.Column(UiNode.Heading("Home"), UiNode.Separator(),
                UiNode.Button("Documents", "files.documents.open"),
                UiNode.Button("Downloads", "files.downloads.open"),
                UiNode.Label("Pippin Files")))];
}

public sealed class Notifications : Application
{
    public override string Id => "org.pippin.notifications";
    public override IEnumerable<Surface> CreateSurfaces() =>
        [new("notifications", SurfaceRole.Notification, "Notifications", 482, 48, 300, 96,
            UiNode.Label("Welcome to Pippin"))];
}

public static class Program
{
    public static async Task Main(string[] args)
    {
        Application[] clients = [new Wallpaper(), new Panel(), new Dock(),
            new Launcher(), new Settings(), new Files(), new Notifications()];
        if (args is ["--bridge", var path])
        {
            await new HostBridge(path, clients).RunAsync();
            return;
        }
        var descriptions = clients.Select(client => new ClientDescription(client.Id,
            client.CreateSurfaces().ToArray())).ToArray();
        Console.WriteLine(JsonSerializer.Serialize(descriptions, new JsonSerializerOptions
        {
            WriteIndented = true,
            Converters = { new JsonStringEnumConverter(JsonNamingPolicy.CamelCase) }
        }));
    }
}

public sealed record ClientDescription(string ApplicationId, IReadOnlyList<Surface> Surfaces);

/// Runs the C# shell on the host while QEMU displays its surfaces. The same
/// client descriptions can later be sent from a managed process in Pippin.
internal sealed class HostBridge(string socketPath, Application[] clients)
{
    private readonly Channel<string> incoming = Channel.CreateUnbounded<string>();
    private StreamWriter? writer;
    private bool alternateWallpaper;

    public async Task RunAsync()
    {
        using var socket = new Socket(AddressFamily.Unix, SocketType.Stream, ProtocolType.Unspecified);
        var deadline = DateTime.UtcNow.AddSeconds(45);
        while (true)
        {
            try { await socket.ConnectAsync(new UnixDomainSocketEndPoint(socketPath)); break; }
            catch (SocketException) when (DateTime.UtcNow < deadline) { await Task.Delay(200); }
        }
        using var stream = new NetworkStream(socket, ownsSocket: false);
        using var reader = new StreamReader(stream);
        writer = new StreamWriter(stream) { AutoFlush = true, NewLine = "\n" };
        _ = Task.Run(async () =>
        {
            while (await reader.ReadLineAsync() is { } line)
                await incoming.Writer.WriteAsync(line);
            incoming.Writer.TryComplete();
        });

        Console.WriteLine("C# shell connected; waiting for Pippin...");
        while (DateTime.UtcNow < deadline)
        {
            await writer.WriteLineAsync("H|1");
            using var timeout = new CancellationTokenSource(400);
            try
            {
                if (await incoming.Reader.ReadAsync(timeout.Token) == "R|1") break;
            }
            catch (OperationCanceledException) { }
        }
        if (DateTime.UtcNow >= deadline) throw new TimeoutException("Pippin did not answer the shell handshake");

        foreach (var name in new[] { "wallpaper", "panel", "dock", "notifications" })
            await ShowAsync(name);
        Console.WriteLine("C# desktop displayed in QEMU. Click Pippin or Files to open apps.");
        await foreach (var line in incoming.Reader.ReadAllAsync())
            if (line.StartsWith("E|", StringComparison.Ordinal)) await HandleAsync(line[2..]);
    }

    private async Task HandleAsync(string action)
    {
        Console.WriteLine("C# action: " + action);
        if (action.EndsWith(".close", StringComparison.Ordinal))
        {
            await SendAsync("X|" + action[..^6]);
            return;
        }
        switch (action)
        {
            case "launcher.open": await ShowAsync("launcher"); break;
            case "settings.open": await ShowAsync("settings"); break;
            case "files.open": await ShowAsync("files"); break;
            case "wallpaper.select.gradient":
                alternateWallpaper = !alternateWallpaper;
                await SendAsync("S|wallpaper|B|0|0|800|600|" +
                    (alternateWallpaper ? "#4b4f63" : "#263746") + "|");
                break;
        }
    }

    private async Task ShowAsync(string name)
    {
        var client = clients.Single(app => app.Id == "org.pippin." + name);
        foreach (var surface in client.CreateSurfaces())
        {
            var role = surface.Role switch
            {
                SurfaceRole.Wallpaper => "B", SurfaceRole.Panel => "P",
                SurfaceRole.Dock => "D", SurfaceRole.Launcher => "L",
                SurfaceRole.Notification => "N", _ => "W"
            };
            var title = surface.Role == SurfaceRole.Wallpaper ? surface.Background : surface.Title;
            var rows = Flatten(surface.Content)
                .Select(node => Safe(node.Text) + "@" + Safe(node.Action));
            await SendAsync(string.Join('|', "S", surface.Id, role, surface.X, surface.Y,
                surface.Width, surface.Height, Safe(title), string.Join(';', rows)));
        }
    }

    private async Task SendAsync(string line)
    {
        await writer!.WriteLineAsync(line);
        var pendingActions = new List<string>();
        while (true)
        {
            var reply = await incoming.Reader.ReadAsync();
            if (reply == "A") break;
            if (reply.StartsWith("E|", StringComparison.Ordinal)) pendingActions.Add(reply[2..]);
        }
        foreach (var action in pendingActions) await HandleAsync(action);
    }

    private static IEnumerable<UiNode> Flatten(UiNode? node)
    {
        if (node is null) yield break;
        if (node.Kind is "label" or "heading" or "button" or "toggle") yield return node;
        if (node.Children is not null)
            foreach (var child in node.Children)
                foreach (var item in Flatten(child)) yield return item;
    }

    private static string Safe(string? text) => new((text ?? "").Where(c =>
        c is >= ' ' and <= '~' && c is not ('|' or ';' or '@')).ToArray());
}
