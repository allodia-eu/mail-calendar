// Real pointer and keyboard input, for the presses a peer cannot make: a double-click, a click into
// the composer's web page, typing into it. Sent from this process, in the physical pixels the agent
// reports, so this process is made DPI aware before anything is measured.

using System.Runtime.CompilerServices;
using System.Runtime.InteropServices;

namespace Allodia.Mailcal.UiTests.Driver;

internal static class Input
{
    [ModuleInitializer]
    internal static void BecomeDpiAware() => SetProcessDpiAwarenessContext(PerMonitorAwareV2);

    /// <summary>
    /// Brings <paramref name="hwnd"/> to the front, unless it is there already: input goes to
    /// whatever is in front. Windows lets a process take the front only when it sent the last input,
    /// so a refused request is followed by a keystroke and asked again. The key is 0xE8, which
    /// Windows assigns to nothing, because it reaches whichever window is in front at the time.
    /// </summary>
    internal static void BringToFront(long hwnd)
    {
        var target = (nint)hwnd;
        if (GetForegroundWindow() == target)
        {
            return;
        }
        if (!SetForegroundWindow(target))
        {
            keybd_event(0xE8, 0, 0, 0);
            keybd_event(0xE8, 0, KeyUp, 0);
            SetForegroundWindow(target);
        }
        var deadline = DateTime.UtcNow.AddSeconds(2);
        while (GetForegroundWindow() != target)
        {
            if (DateTime.UtcNow > deadline)
            {
                throw new InvalidOperationException($"window {hwnd} would not come to the front");
            }
            Thread.Sleep(20);
        }
    }

    /// <summary>Clicks the middle of <paramref name="element"/>, once or as a double-click, with its
    /// window in front.</summary>
    internal static void Click(Element element, int times = 1)
    {
        if (!element.Rendered)
        {
            throw new InvalidOperationException($"'{element.Name}' (#{element.Id}) is not drawn, so a press cannot land on it");
        }
        BringToFront(element.Window);
        ClickAt((int)(element.X + element.Width / 2), (int)(element.Y + element.Height / 2), times);
    }

    /// <summary>Clicks a screen point, in physical pixels, once or as a double-click.</summary>
    internal static void ClickAt(int x, int y, int times = 1)
    {
        SetCursorPos(x, y);
        for (var i = 0; i < times; i++)
        {
            Send(Mouse(MouseLeftDown), Mouse(MouseLeftUp));
        }
    }

    /// <summary>Types <paramref name="text"/> into whatever has the keyboard, as characters rather
    /// than keys, so the layout does not matter.</summary>
    internal static void Type(string text)
    {
        foreach (var unit in text)
        {
            Send(Key(unit, KeyUnicode), Key(unit, KeyUnicode | KeyUp));
        }
    }

    /// <summary>The display scale of <paramref name="hwnd"/>: 1.0 at 100%, 2.0 at 200%.</summary>
    internal static double Scale(long hwnd) => GetDpiForWindow((nint)hwnd) / 96.0;

    private static void Send(params ReadOnlySpan<NativeInput> inputs)
    {
        var sent = SendInput((uint)inputs.Length, inputs.ToArray(), Marshal.SizeOf<NativeInput>());
        if (sent != inputs.Length)
        {
            throw new InvalidOperationException($"Windows took {sent} of {inputs.Length} input events; is a window of higher integrity in front?");
        }
    }

    private static NativeInput Mouse(uint flags) =>
        new() { Type = InputMouse, Union = new InputUnion { Mouse = new MouseInput { Flags = flags } } };

    private static NativeInput Key(char unit, uint flags) =>
        new() { Type = InputKeyboard, Union = new InputUnion { Keyboard = new KeyboardInput { Scan = unit, Flags = flags } } };

    private const uint InputMouse = 0;
    private const uint InputKeyboard = 1;
    private const uint MouseLeftDown = 0x0002;
    private const uint MouseLeftUp = 0x0004;
    private const uint KeyUp = 0x0002;
    private const uint KeyUnicode = 0x0004;
    private static readonly nint PerMonitorAwareV2 = -4;

    [StructLayout(LayoutKind.Sequential)]
    private struct NativeInput
    {
        public uint Type;
        public InputUnion Union;
    }

    [StructLayout(LayoutKind.Explicit)]
    private struct InputUnion
    {
        [FieldOffset(0)] public MouseInput Mouse;
        [FieldOffset(0)] public KeyboardInput Keyboard;
    }

    [StructLayout(LayoutKind.Sequential)]
    private struct MouseInput
    {
        public int Dx;
        public int Dy;
        public uint Data;
        public uint Flags;
        public uint Time;
        public nint Extra;
    }

    [StructLayout(LayoutKind.Sequential)]
    private struct KeyboardInput
    {
        public ushort Virtual;
        public ushort Scan;
        public uint Flags;
        public uint Time;
        public nint Extra;
    }

    [DllImport("user32.dll")] private static extern uint SendInput(uint count, NativeInput[] inputs, int size);
    [DllImport("user32.dll")] private static extern bool SetCursorPos(int x, int y);
    [DllImport("user32.dll")] private static extern bool SetForegroundWindow(nint hwnd);
    [DllImport("user32.dll")] private static extern nint GetForegroundWindow();
    [DllImport("user32.dll")] private static extern void keybd_event(byte vk, byte scan, uint flags, nuint extra);
    [DllImport("user32.dll")] private static extern uint GetDpiForWindow(nint hwnd);
    [DllImport("user32.dll")] private static extern bool SetProcessDpiAwarenessContext(nint context);
}
