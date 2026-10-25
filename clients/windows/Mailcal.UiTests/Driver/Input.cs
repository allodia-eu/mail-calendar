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
    /// so a refused request is followed by a keystroke and asked again, until it is granted. The key
    /// is 0xE8, which Windows assigns to nothing, because it reaches whichever window is in front
    /// at the time. Once is not always enough: a window the app has just opened can take the front
    /// back between the keystroke and the request.
    /// </summary>
    internal static void BringToFront(long hwnd)
    {
        var target = (nint)hwnd;
        var deadline = DateTime.UtcNow.AddSeconds(5);
        while (GetForegroundWindow() != target)
        {
            if (DateTime.UtcNow > deadline)
            {
                throw new InvalidOperationException($"window {hwnd} would not come to the front");
            }
            if (!SetForegroundWindow(target))
            {
                keybd_event(0xE8, 0, 0, 0);
                keybd_event(0xE8, 0, KeyUp, 0);
                SetForegroundWindow(target);
            }
            Thread.Sleep(100);
        }
    }

    /// <summary>Clicks the middle of <paramref name="element"/>, once or as a double-click, with its
    /// window in front; <paramref name="right"/> presses the secondary button instead.</summary>
    internal static void Click(Element element, int times = 1, bool right = false)
    {
        if (!element.Rendered)
        {
            throw new InvalidOperationException($"'{element.Name}' (#{element.Id}) is not drawn, so a press cannot land on it");
        }
        BringToFront(element.Window);
        ClickAt((int)(element.X + element.Width / 2), (int)(element.Y + element.Height / 2), times, right);
    }

    /// <summary>Clicks a screen point, in physical pixels, once or as a double-click.</summary>
    internal static void ClickAt(int x, int y, int times = 1, bool right = false)
    {
        SetCursorPos(x, y);
        var (down, up) = right ? (MouseRightDown, MouseRightUp) : (MouseLeftDown, MouseLeftUp);
        for (var i = 0; i < times; i++)
        {
            Send(Mouse(down), Mouse(up));
        }
    }

    /// <summary>
    /// Drags <paramref name="from"/> to a screen point and lets go there: the button pressed on the
    /// element, the pointer carried across in steps, as a hand moves it, and a rest at the point
    /// before the release, so whatever is under it has answered the drag before the drop arrives.
    /// The moves are real input rather than cursor jumps, which is what the system's drag loop reads.
    /// </summary>
    internal static void Drag(Element from, int toX, int toY, int dwellMs = 800)
    {
        if (!from.Rendered)
        {
            throw new InvalidOperationException($"'{from.Name}' (#{from.Id}) is not drawn, so a drag cannot start on it");
        }
        BringToFront(from.Window);
        var (x, y) = ((int)(from.X + from.Width / 2), (int)(from.Y + from.Height / 2));
        MoveTo(x, y);
        Send(Mouse(MouseLeftDown));
        Thread.Sleep(100);
        const int steps = 25;
        for (var i = 1; i <= steps; i++)
        {
            MoveTo(x + ((toX - x) * i / steps), y + ((toY - y) * i / steps));
            Thread.Sleep(20);
        }
        Thread.Sleep(dwellMs);
        // Real input shares the desktop with the person at it: a hand on the mouse or another window
        // taking the front swallows the drag without a trace in the app. Read both before letting go,
        // and let go regardless, so a drag spoiled from outside says so rather than reading as a
        // drop the app ignored. The absolute move rounds, hence the pixel of slack.
        var moved = GetCursorPos(out var at) && (Math.Abs(at.X - toX) > 2 || Math.Abs(at.Y - toY) > 2);
        var behind = GetForegroundWindow() != (nint)from.Window;
        Send(Mouse(MouseLeftUp));
        if (moved)
        {
            throw new InvalidOperationException($"the pointer was moved during the drag (at {at.X},{at.Y} rather than {toX},{toY}): input from outside the test, so the drop proves nothing");
        }
        if (behind)
        {
            throw new InvalidOperationException("another window came to the front during the drag, so the drop proves nothing");
        }
    }

    /// <summary>Moves the pointer to a screen point, in physical pixels, as a mouse moves it.</summary>
    private static void MoveTo(int x, int y)
    {
        var (left, top) = (GetSystemMetrics(VirtualScreenLeft), GetSystemMetrics(VirtualScreenTop));
        var (width, height) = (GetSystemMetrics(VirtualScreenWidth), GetSystemMetrics(VirtualScreenHeight));
        Send(new NativeInput
        {
            Type = InputMouse,
            Union = new InputUnion
            {
                Mouse = new MouseInput
                {
                    Dx = (int)((x - left) * 65535L / (width - 1)),
                    Dy = (int)((y - top) * 65535L / (height - 1)),
                    Flags = MouseMove | MouseAbsolute | MouseVirtualDesk,
                },
            },
        });
    }

    /// <summary>Presses and releases one key, by its virtual-key code, in whatever has the keyboard.
    /// With its scan code too, as a keyboard sends it: a text box reads that, not the code alone.</summary>
    internal static void Press(ushort virtualKey)
    {
        var scan = (ushort)MapVirtualKeyW(virtualKey, MapVirtualKeyToScanCode);
        Send(
            new NativeInput { Type = InputKeyboard, Union = new InputUnion { Keyboard = new KeyboardInput { Virtual = virtualKey, Scan = scan } } },
            new NativeInput { Type = InputKeyboard, Union = new InputUnion { Keyboard = new KeyboardInput { Virtual = virtualKey, Scan = scan, Flags = KeyUp } } });
    }

    private const uint MapVirtualKeyToScanCode = 0;

    internal const ushort Escape = 0x1B;
    internal const ushort Enter = 0x0D;

    /// <summary>Turns the wheel over a screen point, in notches: positive is away from the user.</summary>
    internal static void Wheel(int x, int y, int notches)
    {
        SetCursorPos(x, y);
        Send(new NativeInput
        {
            Type = InputMouse,
            Union = new InputUnion { Mouse = new MouseInput { Data = unchecked((uint)(notches * 120)), Flags = MouseWheel } },
        });
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
    private const uint MouseRightDown = 0x0008;
    private const uint MouseRightUp = 0x0010;
    private const uint MouseWheel = 0x0800;
    private const uint MouseMove = 0x0001;
    private const uint MouseAbsolute = 0x8000;
    private const uint MouseVirtualDesk = 0x4000;
    private const int VirtualScreenLeft = 76;
    private const int VirtualScreenTop = 77;
    private const int VirtualScreenWidth = 78;
    private const int VirtualScreenHeight = 79;
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

    [StructLayout(LayoutKind.Sequential)]
    private struct CursorPoint
    {
        public int X;
        public int Y;
    }

    [DllImport("user32.dll")] private static extern uint SendInput(uint count, NativeInput[] inputs, int size);
    [DllImport("user32.dll")] private static extern bool SetCursorPos(int x, int y);
    [DllImport("user32.dll")] private static extern bool GetCursorPos(out CursorPoint point);
    [DllImport("user32.dll")] private static extern bool SetForegroundWindow(nint hwnd);
    [DllImport("user32.dll")] private static extern nint GetForegroundWindow();
    [DllImport("user32.dll")] private static extern void keybd_event(byte vk, byte scan, uint flags, nuint extra);
    [DllImport("user32.dll")] private static extern uint GetDpiForWindow(nint hwnd);
    [DllImport("user32.dll")] private static extern uint MapVirtualKeyW(uint code, uint mapType);
    [DllImport("user32.dll")] private static extern bool SetProcessDpiAwarenessContext(nint context);
    [DllImport("user32.dll")] private static extern int GetSystemMetrics(int index);
}
