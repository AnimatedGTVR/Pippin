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
        [new("wallpaper", SurfaceRole.Wallpaper, "Wallpaper", 0, 0, 1024, 768,
            Background: "#2f80ed")];
}

public sealed class Panel : Application
{
    public override string Id => "org.pippin.panel";
    public override IEnumerable<Surface> CreateSurfaces() =>
        [new("panel", SurfaceRole.Panel, "Panel", 0, 0, 1024, 44,
            UiNode.Row(UiNode.Button("Pippin", "launcher.open"),
                UiNode.Button("Apps", "launcher.open"),
                UiNode.Button("Files", "files.open"),
                UiNode.Button("Settings", "settings.open"),
                UiNode.Label("WiFi  Sound  Battery")))];
}

public sealed class Dock : Application
{
    public override string Id => "org.pippin.dock";
    public override IEnumerable<Surface> CreateSurfaces() =>
        [new("dock", SurfaceRole.Dock, "Dock", 330, 696, 364, 58,
            UiNode.Row(UiNode.Button("Apps", "launcher.restore"),
                UiNode.Button("Files", "files.restore"),
                UiNode.Button("Settings", "settings.restore")))];
}

public sealed class Launcher : Application
{
    public override string Id => "org.pippin.launcher";
    public override IEnumerable<Surface> CreateSurfaces() =>
        [new("launcher", SurfaceRole.Launcher, "Applications", 24, 62, 340, 470,
            UiNode.Column(UiNode.Heading("Applications"),
                UiNode.Search("Search apps", "launcher.search"), UiNode.Separator(),
                UiNode.Card(UiNode.Button("Files", "files.open"),
                    UiNode.Label("Browse your files")),
                UiNode.Card(UiNode.Button("Settings", "settings.open"),
                    UiNode.Label("Configure Pippin"))))];
}

public sealed class Settings : Application
{
    public override string Id => "org.pippin.settings";
    public override IEnumerable<Surface> CreateSurfaces() =>
        [new("settings", SurfaceRole.AppWindow, "Settings", 272, 116, 520, 430,
            UiNode.Column(UiNode.Heading("Settings"), UiNode.Separator(),
                UiNode.Card(UiNode.Heading("Appearance"),
                    UiNode.Button("Change wallpaper", "wallpaper.select.gradient"),
                    UiNode.Toggle("Animations", "settings.animations.toggle")),
                UiNode.Card(UiNode.Heading("Desktop"),
                    UiNode.Toggle("Show dock", "settings.dock.toggle"),
                    UiNode.Toggle("Notifications", "settings.notifications.toggle"))))];
}

public sealed class Files : Application
{
    public override string Id => "org.pippin.files";
    public override IEnumerable<Surface> CreateSurfaces() =>
        [new("files", SurfaceRole.AppWindow, "Files", 100, 94, 620, 500,
            UiNode.Column(UiNode.Heading("Home"),
                UiNode.Search("Search files", "files.search"), UiNode.Separator(),
                UiNode.Card(UiNode.Button("Documents", "files.documents.open"),
                    UiNode.Label("Documents")),
                UiNode.Card(UiNode.Button("Downloads", "files.downloads.open"),
                    UiNode.Label("Downloads"))))];
}

public sealed class TerminalApp : Application
{
    public override string Id => "org.pippin.terminal";
    public override IEnumerable<Surface> CreateSurfaces() =>
        [new("terminal", SurfaceRole.AppWindow, "Terminal", 190, 130, 640, 420,
            UiNode.Column(UiNode.Heading("Pippin Terminal"),
                UiNode.Label("pippin> ready"),
                UiNode.Label("Alt+T opens this terminal")))];
}

public sealed class Notifications : Application
{
    public override string Id => "org.pippin.notifications";
    public override IEnumerable<Surface> CreateSurfaces() =>
        [new("notifications", SurfaceRole.Notification, "Notifications", 710, 54, 300, 96,
            UiNode.Label("Welcome to Pippin"))];
}

public static class Program
{
    public static async Task Main(string[] args)
    {
        Application[] clients = [new Wallpaper(), new Panel(), new Dock(),
            new Launcher(), new Settings(), new Files(), new TerminalApp(), new Notifications()];
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
    private bool dockVisible = true;
    private bool notificationsEnabled = true;
    private bool animationsEnabled = true;

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
            case "terminal.open": await ShowAsync("terminal"); break;
            case "launcher.restore": await RestoreOrShowAsync("launcher"); break;
            case "settings.restore": await RestoreOrShowAsync("settings"); break;
            case "files.restore": await RestoreOrShowAsync("files"); break;
            case "settings.animations.toggle":
                animationsEnabled = !animationsEnabled;
                await ShowStatusAsync("Animations " + (animationsEnabled ? "on" : "off"));
                break;
            case "settings.dock.toggle":
                dockVisible = !dockVisible;
                if (dockVisible) await ShowAsync("dock"); else await SendAsync("X|dock");
                break;
            case "settings.notifications.toggle":
                notificationsEnabled = !notificationsEnabled;
                if (notificationsEnabled) await ShowAsync("notifications"); else await SendAsync("X|notifications");
                break;
            case "files.documents.open": await ShowStatusAsync("Documents"); break;
            case "files.downloads.open": await ShowStatusAsync("Downloads"); break;
            case "launcher.search": await ShowStatusAsync("App search ready"); break;
            case "files.search": await ShowStatusAsync("File search ready"); break;
            case "wallpaper.select.gradient":
                await SendAsync("S|wallpaper|B|0|0|1024|768|#2f80ed|");
                await ShowStatusAsync("Blue wallpaper applied");
                break;
        }
    }

    private async Task ShowStatusAsync(string message)
    {
        if (!notificationsEnabled) return;
        await SendAsync("S|notifications|N|482|48|300|96|Notifications|" + Safe(message) + "@");
    }

    private async Task RestoreOrShowAsync(string name)
    {
        await SendAsync("R|" + name);
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
                .Select(node => NodePrefix(node.Kind) + Safe(node.Text) + "@" + Safe(node.Action));
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
        if (node.Kind is "label" or "heading" or "button" or "toggle" or "search") yield return node;
        if (node.Children is not null)
            foreach (var child in node.Children)
                foreach (var item in Flatten(child)) yield return item;
    }

    private static string NodePrefix(string kind) => kind switch
    {
        "heading" => "h:", "search" => "s:", "toggle" => "t:",
        "button" => "b:", _ => "l:"
    };

    private static string Safe(string? text) => new((text ?? "").Where(c =>
        c is >= ' ' and <= '~' && c is not ('|' or ';' or '@')).ToArray());
}
