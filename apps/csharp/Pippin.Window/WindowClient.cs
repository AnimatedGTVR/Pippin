using System.Globalization;
using System.Net.Sockets;
using System.Threading.Channels;

namespace Pippin.Windowing;

public sealed record WindowEvent(uint WindowId, string Kind, int A, int B);

/// Low-level client API. Only this class knows the broker wire protocol.
public sealed class WindowClient : IAsyncDisposable
{
    private readonly Socket socket;
    private readonly NetworkStream stream;
    private readonly StreamReader reader;
    private readonly StreamWriter writer;
    private readonly SemaphoreSlim sendLock = new(1);
    private readonly Channel<string> replies = Channel.CreateUnbounded<string>();
    private readonly Channel<WindowEvent> events = Channel.CreateUnbounded<WindowEvent>();
    private uint nextId;

    private WindowClient(Socket socket)
    {
        this.socket = socket;
        stream = new NetworkStream(socket, ownsSocket: false);
        reader = new StreamReader(stream);
        writer = new StreamWriter(stream) { AutoFlush = true, NewLine = "\n" };
        nextId = (uint)Random.Shared.Next(1, int.MaxValue);
        _ = Task.Run(ReadLoopAsync);
    }

    public static async Task<WindowClient> ConnectAsync(string socketPath, CancellationToken cancellation = default)
    {
        var socket = new Socket(AddressFamily.Unix, SocketType.Stream, ProtocolType.Unspecified);
        var deadline = DateTime.UtcNow.AddSeconds(45);
        while (true)
        {
            try
            {
                await socket.ConnectAsync(new UnixDomainSocketEndPoint(socketPath), cancellation);
                return new WindowClient(socket);
            }
            catch (SocketException) when (DateTime.UtcNow < deadline)
            {
                await Task.Delay(200, cancellation);
            }
        }
    }

    public IAsyncEnumerable<WindowEvent> Events(CancellationToken cancellation = default) =>
        events.Reader.ReadAllAsync(cancellation);

    public async Task<uint> CreateWindowAsync(string title, int x, int y, int width, int height)
    {
        if (width is < 80 or > 600 || height is < 50 or > 440) throw new ArgumentOutOfRangeException(nameof(width));
        var id = Interlocked.Increment(ref nextId);
        await SendAsync($"WC|{id}|{x}|{y}|{width}|{height}|{Safe(title)}");
        return id;
    }

    public Task DestroyWindowAsync(uint id) => SendAsync($"WD|{id}");
    public Task ShowWindowAsync(uint id) => SendAsync($"WS|{id}");
    public Task HideWindowAsync(uint id) => SendAsync($"WH|{id}");
    public Task SetTitleAsync(uint id, string title) => SendAsync($"WT|{id}|{Safe(title)}");
    public Task SetSizeAsync(uint id, int width, int height) => SendAsync($"WZ|{id}|{width}|{height}");
    public Task SetPositionAsync(uint id, int x, int y) => SendAsync($"WM|{id}|{x}|{y}");
    public Task InvalidateAsync(uint id) => SendAsync($"WI|{id}");
    public Task SetCursorAsync(uint id, string cursor) => SendAsync($"WK|{id}|{Safe(cursor)}");

    /// Submit the application's RGB pixels as bounded, run-length encoded
    /// chunks. Rust stores them in this window's private surface buffer.
    public async Task SubmitSurfaceAsync(uint id, ReadOnlyMemory<uint> pixels)
    {
        await SendAsync($"WB|{id}");
        var source = pixels.Span.ToArray();
        var offset = 0;
        while (offset < source.Length)
        {
            var start = offset;
            var parts = new List<string>();
            var chars = 0;
            while (offset < source.Length && parts.Count < 22)
            {
                var color = source[offset] & 0x00ffffff;
                var count = 1;
                while (offset + count < source.Length && source[offset + count] == source[offset]
                    && count < 65535) count++;
                var part = count.ToString(CultureInfo.InvariantCulture) + "," +
                    color.ToString("X6", CultureInfo.InvariantCulture);
                if (parts.Count > 0 && chars + part.Length + 1 > 320) break;
                parts.Add(part);
                chars += part.Length + 1;
                offset += count;
            }
            await SendAsync($"WP|{id}|{start}|{string.Join(';', parts)}");
        }
        await SendAsync($"WE|{id}");
    }

    private async Task SendAsync(string line)
    {
        if (line.Length > 380) throw new ArgumentException("Window protocol message too long");
        await sendLock.WaitAsync();
        try
        {
            await writer.WriteLineAsync(line);
            var reply = await replies.Reader.ReadAsync();
            if (reply != "OK") throw new InvalidOperationException("Window command failed: " + reply);
        }
        finally { sendLock.Release(); }
    }

    private async Task ReadLoopAsync()
    {
        try
        {
            while (await reader.ReadLineAsync() is { } line)
            {
                if (line.StartsWith("EV|", StringComparison.Ordinal))
                {
                    var fields = line.Split('|');
                    if (fields.Length == 5 && uint.TryParse(fields[1], out var id)
                        && int.TryParse(fields[3], out var a) && int.TryParse(fields[4], out var b))
                        await events.Writer.WriteAsync(new WindowEvent(id, fields[2], a, b));
                }
                else await replies.Writer.WriteAsync(line);
            }
        }
        catch (IOException) { }
        finally { events.Writer.TryComplete(); replies.Writer.TryComplete(); }
    }

    private static string Safe(string text) => new(text.Where(c => c is >= ' ' and <= '~' && c != '|').Take(48).ToArray());

    public async ValueTask DisposeAsync()
    {
        await writer.DisposeAsync();
        reader.Dispose();
        stream.Dispose();
        socket.Dispose();
        sendLock.Dispose();
    }
}
