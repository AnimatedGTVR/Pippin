using System.Collections.Concurrent;
using System.Net.Sockets;
using System.Threading.Channels;

namespace Pippin.Broker;

public static class Program
{
    public static async Task Main(string[] args)
    {
        if (args.Length != 2) throw new ArgumentException("Usage: Pippin.Broker QEMU_SOCKET CLIENT_SOCKET");
        await new Broker(args[0], args[1]).RunAsync();
    }
}

/// Multiplexes independent host processes onto COM2. It owns the mapping
/// from each global window ID to its client connection.
internal sealed class Broker(string qemuPath, string clientPath)
{
    private readonly ConcurrentDictionary<uint, Client> owners = new();
    private readonly Channel<string> acknowledgements = Channel.CreateUnbounded<string>();
    private readonly SemaphoreSlim guestLock = new(1);
    private StreamWriter? guestWriter;

    public async Task RunAsync()
    {
        using var guestSocket = new Socket(AddressFamily.Unix, SocketType.Stream, ProtocolType.Unspecified);
        var deadline = DateTime.UtcNow.AddSeconds(45);
        while (true)
        {
            try { await guestSocket.ConnectAsync(new UnixDomainSocketEndPoint(qemuPath)); break; }
            catch (SocketException) when (DateTime.UtcNow < deadline) { await Task.Delay(200); }
        }
        using var guestStream = new NetworkStream(guestSocket, ownsSocket: false);
        using var guestReader = new StreamReader(guestStream);
        guestWriter = new StreamWriter(guestStream) { AutoFlush = true, NewLine = "\n" };
        _ = Task.Run(async () =>
        {
            while (await guestReader.ReadLineAsync() is { } line)
            {
                if (line.StartsWith("EV|", StringComparison.Ordinal))
                {
                    var fields = line.Split('|');
                    if (fields.Length >= 3 && uint.TryParse(fields[1], out var id)
                        && owners.TryGetValue(id, out var client))
                    {
                        try { await client.SendAsync(line); }
                        catch (IOException) { owners.TryRemove(id, out _); }
                    }
                }
                else await acknowledgements.Writer.WriteAsync(line);
            }
            acknowledgements.Writer.TryComplete();
        });

        Console.WriteLine("Window broker connected; waiting for Pippin protocol v2...");
        while (DateTime.UtcNow < deadline)
        {
            await guestWriter.WriteLineAsync("H|2");
            using var timeout = new CancellationTokenSource(400);
            try { if (await acknowledgements.Reader.ReadAsync(timeout.Token) == "R|2") break; }
            catch (OperationCanceledException) { }
        }
        if (DateTime.UtcNow >= deadline) throw new TimeoutException("Pippin window protocol did not answer");

        if (File.Exists(clientPath)) File.Delete(clientPath);
        using var listener = new Socket(AddressFamily.Unix, SocketType.Stream, ProtocolType.Unspecified);
        listener.Bind(new UnixDomainSocketEndPoint(clientPath));
        listener.Listen(16);
        Console.WriteLine("Window broker ready: " + clientPath);
        try
        {
            while (true)
            {
                var socket = await listener.AcceptAsync();
                _ = Task.Run(() => HandleClientAsync(socket));
            }
        }
        finally { if (File.Exists(clientPath)) File.Delete(clientPath); }
    }

    private async Task HandleClientAsync(Socket socket)
    {
        await using var client = new Client(socket);
        Console.WriteLine("Window client connected");
        try
        {
            while (await client.ReadAsync() is { } line)
            {
                var fields = line.Split('|', 3);
                if (fields.Length < 2 || !uint.TryParse(fields[1], out var id) || id == 0)
                { await client.SendAsync("ERR|BadWindowId"); continue; }
                var create = fields[0] == "WC";
                if (create ? !owners.TryAdd(id, client)
                           : !owners.TryGetValue(id, out var owner) || owner != client)
                { await client.SendAsync("ERR|WindowOwnership"); continue; }
                try
                {
                    await SendGuestAsync(line);
                    await client.SendAsync("OK");
                    if (fields[0] == "WD") owners.TryRemove(id, out _);
                }
                catch (InvalidOperationException)
                {
                    if (create) owners.TryRemove(id, out _);
                    await client.SendAsync("ERR|Rejected");
                }
                catch
                {
                    if (create) owners.TryRemove(id, out _);
                    await client.SendAsync("ERR|GuestDisconnected");
                }
            }
        }
        catch (IOException) { }
        finally
        {
            foreach (var (id, owner) in owners.Where(entry => entry.Value == client).ToArray())
            {
                owners.TryRemove(id, out _);
                try { await SendGuestAsync($"WD|{id}"); } catch { }
            }
            Console.WriteLine("Window client disconnected");
        }
    }

    private async Task SendGuestAsync(string line)
    {
        if (line.Length > 380) throw new ArgumentException("Window message too long");
        await guestLock.WaitAsync();
        try
        {
            await guestWriter!.WriteLineAsync(line);
            var reply = await acknowledgements.Reader.ReadAsync();
            if (reply == "N") throw new InvalidOperationException("Guest rejected window command");
            if (reply != "A") throw new IOException("Unexpected guest reply: " + reply);
        }
        finally { guestLock.Release(); }
    }
}

internal sealed class Client : IAsyncDisposable
{
    private readonly Socket socket;
    private readonly NetworkStream stream;
    private readonly StreamReader reader;
    private readonly StreamWriter writer;
    private readonly SemaphoreSlim writeLock = new(1);

    public Client(Socket socket)
    {
        this.socket = socket;
        stream = new NetworkStream(socket, ownsSocket: false);
        reader = new StreamReader(stream);
        writer = new StreamWriter(stream) { AutoFlush = true, NewLine = "\n" };
    }

    public Task<string?> ReadAsync() => reader.ReadLineAsync();

    public async Task SendAsync(string line)
    {
        await writeLock.WaitAsync();
        try { await writer.WriteLineAsync(line); }
        finally { writeLock.Release(); }
    }

    public async ValueTask DisposeAsync()
    {
        await writer.DisposeAsync();
        reader.Dispose();
        stream.Dispose();
        socket.Dispose();
        writeLock.Dispose();
    }
}
