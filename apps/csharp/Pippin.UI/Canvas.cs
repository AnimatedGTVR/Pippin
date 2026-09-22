namespace Pippin.UI;

public readonly record struct Rect(int X, int Y, int Width, int Height)
{
    public bool Contains(int x, int y) => x >= X && y >= Y && x < X + Width && y < Y + Height;
}

/// Software RGB surface owned by the application, not the compositor.
public sealed class Canvas(int width, int height)
{
    public int Width { get; } = width;
    public int Height { get; } = height;
    public uint[] Pixels { get; } = new uint[checked(width * height)];

    public void Fill(uint color) => Array.Fill(Pixels, color & 0x00ffffff);

    public void FillRect(Rect rect, uint color)
    {
        var x0 = Math.Clamp(rect.X, 0, Width);
        var y0 = Math.Clamp(rect.Y, 0, Height);
        var x1 = Math.Clamp(rect.X + rect.Width, 0, Width);
        var y1 = Math.Clamp(rect.Y + rect.Height, 0, Height);
        for (var y = y0; y < y1; y++)
            Array.Fill(Pixels, color & 0x00ffffff, y * Width + x0, x1 - x0);
    }

    public void DrawText(int x, int y, string text, uint color, int scale = 2)
    {
        foreach (var ch in text.ToUpperInvariant())
        {
            var glyph = Font.Glyph(ch);
            for (var row = 0; row < 7; row++)
                for (var col = 0; col < 5; col++)
                    if ((glyph[row] & (1 << (4 - col))) != 0)
                        FillRect(new Rect(x + col * scale, y + row * scale, scale, scale), color);
            x += 6 * scale;
        }
    }
}

internal static class Font
{
    private static readonly Dictionary<char, byte[]> Glyphs = new()
    {
        ['A']=[14,17,17,31,17,17,17], ['B']=[30,17,17,30,17,17,30],
        ['C']=[14,17,16,16,16,17,14], ['D']=[30,17,17,17,17,17,30],
        ['E']=[31,16,16,30,16,16,31], ['F']=[31,16,16,30,16,16,16],
        ['G']=[14,17,16,23,17,17,14], ['H']=[17,17,17,31,17,17,17],
        ['I']=[31,4,4,4,4,4,31], ['J']=[7,2,2,2,18,18,12],
        ['K']=[17,18,20,24,20,18,17], ['L']=[16,16,16,16,16,16,31],
        ['M']=[17,27,21,21,17,17,17], ['N']=[17,25,21,19,17,17,17],
        ['O']=[14,17,17,17,17,17,14], ['P']=[30,17,17,30,16,16,16],
        ['Q']=[14,17,17,17,21,18,13], ['R']=[30,17,17,30,20,18,17],
        ['S']=[15,16,16,14,1,1,30], ['T']=[31,4,4,4,4,4,4],
        ['U']=[17,17,17,17,17,17,14], ['V']=[17,17,17,17,17,10,4],
        ['W']=[17,17,17,21,21,21,10], ['X']=[17,17,10,4,10,17,17],
        ['Y']=[17,17,10,4,4,4,4], ['Z']=[31,1,2,4,8,16,31],
        ['0']=[14,17,19,21,25,17,14], ['1']=[4,12,4,4,4,4,14],
        ['2']=[14,17,1,2,4,8,31], ['3']=[30,1,1,14,1,1,30],
        ['4']=[2,6,10,18,31,2,2], ['5']=[31,16,16,30,1,1,30],
        ['6']=[14,16,16,30,17,17,14], ['7']=[31,1,2,4,8,8,8],
        ['8']=[14,17,17,14,17,17,14], ['9']=[14,17,17,15,1,1,14],
        ['.']=[0,0,0,0,0,12,12], [':']=[0,12,12,0,12,12,0],
        ['-']=[0,0,0,31,0,0,0], ['/']=[1,1,2,4,8,16,16],
        ['(']=[2,4,8,8,8,4,2], [')']=[8,4,2,2,2,4,8]
    };

    public static byte[] Glyph(char ch) => Glyphs.TryGetValue(ch, out var glyph) ? glyph : [0,0,0,0,0,0,0];
}
