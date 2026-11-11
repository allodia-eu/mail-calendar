// The two ways the agent waits, for a condition and for the windows to stop changing, and the
// screenshot it draws: none of them a sleep the caller has to guess.

#if DEBUG
using System;
using System.Diagnostics;
using System.IO;
using System.Linq;
using System.Runtime.InteropServices.WindowsRuntime;
using System.Text;
using System.Text.Json.Nodes;
using System.Threading;
using System.Threading.Tasks;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation.Peers;
using Microsoft.UI.Xaml.Media.Imaging;
using Windows.Graphics.Imaging;

namespace Allodia.Mailcal.Services;

internal static partial class TestAgent
{
    /// <summary>
    /// Waits until the filter finds something (or, with <c>absent</c>, nothing), checking on every
    /// turn of the UI thread rather than on a timer a caller has to guess. Running out is the
    /// <c>timeout</c> refusal, saying what was near, or for an absence what was still there.
    /// </summary>
    private static JsonObject Wait(JsonObject request)
    {
        var absent = Flag(request, "absent") ?? false;
        var timeout = TimeSpan.FromMilliseconds(Number(request, "timeoutMs") ?? 10_000);
        var filter = request.DeepClone().AsObject();
        filter.Remove("absent");
        filter.Remove("timeoutMs");
        var watch = Stopwatch.StartNew();
        while (true)
        {
            var answer = OnUi(() => Query(filter));
            if (answer["ok"] is not null)
            {
                return answer;
            }
            var found = answer["elements"]!.AsArray();
            if ((found.Count > 0) != absent)
            {
                answer["ms"] = (int)watch.ElapsedMilliseconds;
                return answer;
            }
            if (watch.Elapsed >= timeout)
            {
                var extra = absent
                    ? new JsonObject { ["elements"] = found.DeepClone() }
                    : OnUi(() => new JsonObject { ["near"] = Near(filter) });
                throw new AgentException(
                    absent ? $"still there after {timeout.TotalMilliseconds:F0} ms" : $"nothing matched within {timeout.TotalMilliseconds:F0} ms",
                    "timeout", extra);
            }
            Thread.Sleep(15);
        }
    }

    /// <summary>
    /// Waits until the windows have looked the same for <c>quietMs</c>, and at least <c>floorMs</c>
    /// has passed: an action whose effect arrives after a round trip has not changed anything yet
    /// when it returns, so an early quiet proves nothing. Gives up at <c>capMs</c>.
    /// </summary>
    private static JsonObject Settle(JsonObject request)
    {
        var quiet = TimeSpan.FromMilliseconds(Number(request, "quietMs") ?? 300);
        var floor = TimeSpan.FromMilliseconds(Number(request, "floorMs") ?? 0);
        var cap = TimeSpan.FromMilliseconds(Number(request, "capMs") ?? 10_000);
        var watch = Stopwatch.StartNew();
        var changed = Stopwatch.StartNew();
        string? last = null;
        while (true)
        {
            var shape = OnUi(() => new JsonObject { ["shape"] = Shape() });
            if (shape["ok"] is not null)
            {
                return shape;
            }
            var now = (string)shape["shape"]!;
            if (now != last)
            {
                last = now;
                changed.Restart();
            }
            var settled = changed.Elapsed >= quiet && watch.Elapsed >= floor;
            if (settled || watch.Elapsed >= cap)
            {
                return new JsonObject { ["settled"] = settled, ["ms"] = (int)watch.ElapsedMilliseconds };
            }
            Thread.Sleep(15);
        }
    }

    /// <summary>What every open window shows, as one comparable string.</summary>
    private static string Shape()
    {
        var shape = new StringBuilder();
        foreach (var peer in Walk(OpenWindows().SelectMany(WindowRoots)))
        {
            shape.Append(peer.GetAutomationControlType()).Append('/').Append(peer.GetAutomationId())
                .Append('/').Append(peer.GetName()).Append(peer.IsOffscreen() ? "/off" : "").Append('\n');
        }
        return shape.ToString();
    }

    /// <summary>Renders the element, or the mailbox's content, to a PNG at <c>path</c>, at the
    /// display's scale. A popup is drawn over the window rather than in it, so it is not in a
    /// capture of the window: pass its element to capture it.</summary>
    private static async Task<JsonObject> Screenshot(JsonObject request)
    {
        var path = Path.GetFullPath(Text(request, "path") ?? throw new AgentException("bad arguments: screenshot needs 'path'", "bad-request"));
        UIElement target;
        if (request["h"] is not null)
        {
            target = (PeerOf(HandleIn(request)) as FrameworkElementAutomationPeer)?.Owner
                ?? throw new AgentException("that element draws nothing of its own to capture", "not-showing");
        }
        else
        {
            target = OpenWindows().FirstOrDefault()?.Content
                ?? throw new AgentException("no window is open", "not-showing");
        }
        var bitmap = new RenderTargetBitmap();
        await bitmap.RenderAsync(target);
        if (bitmap.PixelWidth == 0 || bitmap.PixelHeight == 0)
        {
            throw new AgentException("the element is not drawn, so there is nothing to capture", "not-showing");
        }
        var pixels = (await bitmap.GetPixelsAsync()).ToArray();
        Directory.CreateDirectory(Path.GetDirectoryName(path)!);
        using (var file = File.Create(path))
        {
            var encoder = await BitmapEncoder.CreateAsync(BitmapEncoder.PngEncoderId, file.AsRandomAccessStream());
            var dpi = 96 * (target.XamlRoot?.RasterizationScale ?? 1.0);
            encoder.SetPixelData(BitmapPixelFormat.Bgra8, BitmapAlphaMode.Premultiplied,
                (uint)bitmap.PixelWidth, (uint)bitmap.PixelHeight, dpi, dpi, pixels);
            await encoder.FlushAsync();
        }
        return new JsonObject { ["path"] = path, ["width"] = bitmap.PixelWidth, ["height"] = bitmap.PixelHeight };
    }
}
#endif
