using Pippin.Windowing;

namespace Pippin.UI;

public sealed class Window(string title, int width, int height)
{
    public string Title { get; set; } = title;
    public int Width { get; internal set; } = width;
    public int Height { get; internal set; } = height;
    public int X { get; set; } = 80;
    public int Y { get; set; } = 80;
    public Widget? Content { get; set; }
    public uint Id { get; internal set; }
    public bool IsOpen { get; internal set; }
    private bool closeRequested;
    public event Action<WindowEvent>? Input;

    public void Close() => closeRequested = true;

    internal async Task RedrawAsync(WindowClient client)
    {
        if (!IsOpen) return;
        var canvas = new Canvas(Width, Height);
        canvas.Fill(0x00f7f7f5);
        Content?.Arrange(new Rect(0, 0, Width, Height));
        Content?.Paint(canvas);
        await client.SubmitSurfaceAsync(Id, canvas.Pixels);
    }

    internal async Task HandleAsync(WindowEvent e, WindowClient client)
    {
        switch (e.Kind)
        {
            case "CloseRequested": await client.DestroyWindowAsync(Id); IsOpen = false; break;
            case "Resize": Width = e.A; Height = e.B; await RedrawAsync(client); break;
            case "Redraw": await RedrawAsync(client); break;
            case "MouseDown": if (Content?.MouseDown(e.A, e.B) == true) await RedrawAsync(client); break;
            case "MouseUp": if (Content?.MouseUp(e.A, e.B) == true) await RedrawAsync(client); break;
        }
        if (closeRequested && IsOpen) { await client.DestroyWindowAsync(Id); IsOpen = false; }
        Input?.Invoke(e);
    }
}

/// One event pump can own any number of windows in a client process.
public sealed class WindowApplication : IAsyncDisposable
{
    private readonly WindowClient client;
    private readonly Dictionary<uint, Window> windows = [];
    private WindowApplication(WindowClient client) => this.client = client;

    public static async Task<WindowApplication> ConnectAsync(string socketPath) =>
        new(await WindowClient.ConnectAsync(socketPath));

    public async Task ShowAsync(Window window)
    {
        window.Id = await client.CreateWindowAsync(window.Title, window.X, window.Y, window.Width, window.Height);
        window.IsOpen = true;
        windows.Add(window.Id, window);
        await window.RedrawAsync(client);
        await client.ShowWindowAsync(window.Id);
    }

    public async Task RunAsync(CancellationToken cancellation = default)
    {
        await foreach (var e in client.Events(cancellation))
        {
            if (windows.TryGetValue(e.WindowId, out var window))
            {
                await window.HandleAsync(e, client);
                if (!window.IsOpen) windows.Remove(window.Id);
            }
            if (windows.Count == 0) return;
        }
    }

    public ValueTask DisposeAsync() => client.DisposeAsync();
}
