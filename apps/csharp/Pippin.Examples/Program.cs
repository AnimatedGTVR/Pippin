using Pippin.UI;

namespace Pippin.Examples;

public static class Program
{
    public static async Task Main(string[] args)
    {
        if (args.Length != 2) throw new ArgumentException("Usage: Pippin.Examples about|tasks CLIENT_SOCKET");
        await using var application = await WindowApplication.ConnectAsync(args[1]);
        var window = args[0] switch
        {
            "about" => About(),
            "tasks" => Tasks(),
            _ => throw new ArgumentException("Unknown example application")
        };
        await application.ShowAsync(window);
        Console.WriteLine($"{args[0]} process created window {window.Id}");
        await application.RunAsync();
    }

    private static Window About()
    {
        var window = new Window("About Pippin", 380, 215) { X = 90, Y = 112 };
        var layout = new StackPanel();
        layout.Add(new Label("Pippin OS"));
        layout.Add(new Label("A small desktop operating system."));
        layout.Add(new Label("Window protocol v2"));
        var ok = new Button("OK");
        ok.Clicked += window.Close;
        layout.Add(ok);
        window.Content = layout;
        return window;
    }

    private static Window Tasks()
    {
        var window = new Window("Task Manager", 430, 282) { X = 300, Y = 176 };
        var layout = new StackPanel();
        layout.Add(new Label("Pippin Task Manager"));
        layout.Add(new Label("Independent C# client process"));
        layout.Add(new Label("Window state belongs to Rust"));
        layout.Add(new Label("Surface pixels belong to this app"));
        var status = new Label("Input event routing ready");
        layout.Add(status);
        var refresh = new Button("Refresh status");
        refresh.Clicked += () => status.Text = "Updated by this application";
        layout.Add(refresh);
        window.Content = layout;
        return window;
    }
}
