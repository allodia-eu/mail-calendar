//! Pointer input for the Linux client's headless test compositor.
//!
//! ```text
//! mailcal-vpointer move <x> <y>
//! mailcal-vpointer click <x> <y> [left|right|middle]
//! mailcal-vpointer drag <x1> <y1> <x2> <y2> [steps]
//! mailcal-vpointer scroll <x> <y> <dy> [dx]
//! ```
//!
//! Coordinates are output pixels, resolved against the compositor's own output, so a caller states
//! what it reads off a screenshot and nothing has to agree about a size.
//!
//! **Why this exists, when `wlrctl` speaks the same protocol.** A seat with no physical pointer,
//! which is every headless compositor, gains its pointer capability only while a virtual pointer
//! is alive. `wlrctl` creates one, sends a single action and exits, so:
//!
//! - the device is gone before a client has bound `wl_pointer` and can be sent anything, and
//! - the cursor position dies with it, so a move in one invocation and a click in the next are two
//!   unrelated pointers and the click lands wherever the compositor's cursor happened to be.
//!
//! Measured on this repository's harness: three `wlrctl pointer` invocations to move and click a
//! control changed **zero** pixels, and one connection doing the same move and click changed
//! 702,956. That is the whole difference. The upstream report
//! (<https://github.com/cage-kiosk/cage/issues/305>) reads it as a wlroots bug, and its
//! reproduction is the same invocation-per-action shape, so treat "virtual pointer does not work
//! on headless" as a claim about that shape rather than about the protocol.
//!
//! So: one connection holds one pointer for the whole gesture, waits for the compositor to attach
//! it before sending anything, and lingers afterwards so the last events are dispatched.

use std::time::Duration;

use wayland_client::{
    Connection, Dispatch, Proxy, QueueHandle,
    protocol::{
        wl_output,
        wl_pointer::{Axis, ButtonState},
        wl_registry, wl_seat,
    },
};
use wayland_protocols_wlr::virtual_pointer::v1::client::{
    zwlr_virtual_pointer_manager_v1::ZwlrVirtualPointerManagerV1,
    zwlr_virtual_pointer_v1::ZwlrVirtualPointerV1,
};

/// Linux's `input-event-codes.h`, which the protocol takes button codes from.
const BTN_LEFT: u32 = 0x110;
const BTN_RIGHT: u32 = 0x111;
const BTN_MIDDLE: u32 = 0x112;

/// How long to wait after creating the pointer before sending anything.
///
/// The compositor has to attach the device and every client has to see the seat gain its pointer
/// capability and bind `wl_pointer`; an event sent before that reaches nothing and is not
/// redelivered. Overridable for a machine slow enough to need more.
fn settle() -> Duration {
    let ms = std::env::var("MAILCAL_VPOINTER_SETTLE_MS")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(400);
    Duration::from_millis(ms)
}

#[derive(Default)]
struct State {
    manager: Option<ZwlrVirtualPointerManagerV1>,
    seat: Option<wl_seat::WlSeat>,
    /// The output's pixel size, from its current mode. `motion_absolute` is expressed against an
    /// extent rather than in pixels, so without this every caller would have to pass the size and
    /// one that passed a stale one would click somewhere plausible and wrong.
    size: Option<(u32, u32)>,
}

impl Dispatch<wl_registry::WlRegistry, ()> for State {
    fn event(
        state: &mut Self,
        registry: &wl_registry::WlRegistry,
        event: wl_registry::Event,
        (): &(),
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        let wl_registry::Event::Global {
            name,
            interface,
            version,
        } = event
        else {
            return;
        };
        match interface.as_str() {
            "zwlr_virtual_pointer_manager_v1" => {
                state.manager = Some(registry.bind(name, version.min(2), qh, ()));
            }
            "wl_seat" => state.seat = Some(registry.bind(name, version.min(7), qh, ())),
            // Only the first output: the test compositor is configured with exactly one, and a
            // caller's coordinates are read off a capture of it.
            "wl_output" if state.size.is_none() => {
                let _ = registry.bind::<wl_output::WlOutput, _, _>(name, version.min(2), qh, ());
            }
            _ => {}
        }
    }
}

impl Dispatch<wl_output::WlOutput, ()> for State {
    fn event(
        state: &mut Self,
        _: &wl_output::WlOutput,
        event: wl_output::Event,
        (): &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let wl_output::Event::Mode { width, height, .. } = event {
            state.size = Some((
                u32::try_from(width).unwrap_or(0),
                u32::try_from(height).unwrap_or(0),
            ));
        }
    }
}

/// The three proxies this sends on and never hears from.
macro_rules! ignore_events {
    ($($proxy:ty),* $(,)?) => {$(
        impl Dispatch<$proxy, ()> for State {
            fn event(
                _: &mut Self,
                _: &$proxy,
                _: <$proxy as Proxy>::Event,
                (): &(),
                _: &Connection,
                _: &QueueHandle<Self>,
            ) {
            }
        }
    )*};
}
ignore_events!(
    wl_seat::WlSeat,
    ZwlrVirtualPointerManagerV1,
    ZwlrVirtualPointerV1,
);

/// A pointer held open for the length of one gesture.
struct Pointer {
    proxy: ZwlrVirtualPointerV1,
    queue: wayland_client::EventQueue<State>,
    state: State,
    size: (u32, u32),
    /// Milliseconds since the gesture started. The protocol wants a timestamp, and a compositor
    /// reads gaps between them, so a drag that reported the same instant throughout would not read
    /// as a drag.
    clock: u32,
}

impl Pointer {
    fn open() -> Result<Self, String> {
        let connection = Connection::connect_to_env()
            .map_err(|e| format!("no Wayland display to drive ({e}); is a session running?"))?;
        let mut queue = connection.new_event_queue();
        let qh = queue.handle();
        connection.display().get_registry(&qh, ());
        let mut state = State::default();
        // Twice: the first announces the globals, the second delivers the output's mode, which
        // arrives only once the output has been bound.
        for _ in 0..2 {
            queue
                .roundtrip(&mut state)
                .map_err(|e| format!("the compositor stopped answering: {e}"))?;
        }

        let manager = state.manager.clone().ok_or_else(|| {
            "this compositor offers no zwlr_virtual_pointer_manager_v1, so pointer input cannot be \
             driven on it. GNOME is one such: run the client on the headless session instead \
             (clients/linux/build-and-run.sh --headless)"
                .to_owned()
        })?;
        let size = state
            .size
            .filter(|(w, h)| *w > 0 && *h > 0)
            .ok_or_else(|| "the compositor reported no output size".to_owned())?;

        let pointer = manager.create_virtual_pointer(state.seat.as_ref(), &qh, ());
        queue
            .roundtrip(&mut state)
            .map_err(|e| format!("the compositor refused a virtual pointer: {e}"))?;
        std::thread::sleep(settle());
        let mut opened = Self {
            proxy: pointer,
            queue,
            state,
            size,
            clock: 0,
        };
        opened.sync()?;
        Ok(opened)
    }

    fn sync(&mut self) -> Result<(), String> {
        self.queue
            .roundtrip(&mut self.state)
            .map(|_| ())
            .map_err(|e| format!("the compositor stopped answering mid-gesture: {e}"))
    }

    /// Advance the gesture clock and let the compositor catch up.
    fn rest(&mut self, ms: u32) -> Result<(), String> {
        self.sync()?;
        std::thread::sleep(Duration::from_millis(u64::from(ms)));
        self.clock += ms;
        Ok(())
    }

    fn move_to(&mut self, x: u32, y: u32) -> Result<(), String> {
        let (width, height) = self.size;
        self.proxy
            .motion_absolute(self.clock, x.min(width), y.min(height), width, height);
        self.proxy.frame();
        self.rest(20)
    }

    fn button(&mut self, code: u32, pressed: bool) -> Result<(), String> {
        let state = if pressed {
            ButtonState::Pressed
        } else {
            ButtonState::Released
        };
        self.proxy.button(self.clock, code, state);
        self.proxy.frame();
        self.rest(60)
    }

    fn scroll(&mut self, axis: Axis, amount: f64) -> Result<(), String> {
        self.proxy.axis(self.clock, axis, amount);
        self.proxy.frame();
        self.rest(40)
    }

    /// Hold the connection open briefly so the compositor dispatches what was just sent. Dropping
    /// straight after the last `frame` destroys the pointer first, and the events go nowhere.
    fn close(mut self) -> Result<(), String> {
        self.sync()?;
        std::thread::sleep(Duration::from_millis(200));
        self.sync()
    }
}

fn button_code(name: Option<&String>) -> Result<u32, String> {
    match name.map_or("left", String::as_str) {
        "left" => Ok(BTN_LEFT),
        "right" => Ok(BTN_RIGHT),
        "middle" => Ok(BTN_MIDDLE),
        other => Err(format!("unknown button '{other}' (left|right|middle)")),
    }
}

fn number<T: std::str::FromStr>(args: &[String], index: usize, what: &str) -> Result<T, String> {
    args.get(index)
        .ok_or_else(|| format!("missing {what}"))?
        .parse()
        .map_err(|_| format!("{what} must be a number, and got '{}'", args[index]))
}

/// Where a drag's intermediate positions fall. A drag delivered as press, one jump and release is
/// not a drag to a client that reads a threshold or a velocity, so the path is walked.
fn drag_path(from: (u32, u32), to: (u32, u32), steps: u32) -> Vec<(u32, u32)> {
    let steps = steps.max(1);
    // Integer arithmetic throughout: a pixel is a whole number, and the last step lands exactly on
    // the target rather than a rounding away from it.
    let lerp = |a: u32, b: u32, step: u32| -> u32 {
        let (a, b) = (i64::from(a), i64::from(b));
        let value = a + (b - a) * i64::from(step) / i64::from(steps);
        u32::try_from(value.max(0)).unwrap_or(0)
    };
    (1..=steps)
        .map(|step| (lerp(from.0, to.0, step), lerp(from.1, to.1, step)))
        .collect()
}

fn run(args: &[String]) -> Result<(), String> {
    let command = args.first().map_or("", String::as_str);
    let mut pointer = Pointer::open()?;
    match command {
        "move" => {
            pointer.move_to(number(args, 1, "x")?, number(args, 2, "y")?)?;
        }
        "click" => {
            let code = button_code(args.get(3))?;
            pointer.move_to(number(args, 1, "x")?, number(args, 2, "y")?)?;
            pointer.button(code, true)?;
            pointer.button(code, false)?;
        }
        "drag" => {
            let from = (number(args, 1, "x1")?, number(args, 2, "y1")?);
            let to = (number(args, 3, "x2")?, number(args, 4, "y2")?);
            let steps = args.get(5).map_or(Ok(20), |_| number(args, 5, "steps"))?;
            pointer.move_to(from.0, from.1)?;
            pointer.button(BTN_LEFT, true)?;
            for (x, y) in drag_path(from, to, steps) {
                pointer.move_to(x, y)?;
            }
            pointer.button(BTN_LEFT, false)?;
        }
        "scroll" => {
            pointer.move_to(number(args, 1, "x")?, number(args, 2, "y")?)?;
            let dy: f64 = number(args, 3, "dy")?;
            if dy != 0.0 {
                pointer.scroll(Axis::VerticalScroll, dy)?;
            }
            let dx: f64 = args.get(4).map_or(Ok(0.0), |_| number(args, 4, "dx"))?;
            if dx != 0.0 {
                pointer.scroll(Axis::HorizontalScroll, dx)?;
            }
        }
        other => {
            return Err(format!(
                "unknown command '{other}' (move|click|drag|scroll)"
            ));
        }
    }
    pointer.close()
}

fn main() -> std::process::ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match run(&args) {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("error: {message}");
            std::process::ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{BTN_LEFT, BTN_MIDDLE, BTN_RIGHT, button_code, drag_path};

    #[test]
    fn a_drag_walks_its_path_and_ends_on_the_target() {
        let path = drag_path((0, 0), (100, 50), 5);
        assert_eq!(path.len(), 5);
        assert_eq!(path.last().copied(), Some((100, 50)));
        // Monotonic, so a client reading a direction sees one.
        assert!(path.windows(2).all(|pair| pair[0].0 <= pair[1].0));
    }

    #[test]
    fn a_drag_with_no_steps_still_reaches_the_target() {
        assert_eq!(drag_path((10, 10), (20, 20), 0), vec![(20, 20)]);
    }

    #[test]
    fn a_drag_backwards_is_a_drag_too() {
        // A swipe from right to left is how a mail row is dismissed, so this is the common case
        // rather than an edge one.
        let path = drag_path((300, 40), (100, 40), 4);
        assert_eq!(path.last().copied(), Some((100, 40)));
        assert!(path.windows(2).all(|pair| pair[0].0 >= pair[1].0));
    }

    #[test]
    fn buttons_resolve_to_their_input_event_codes() {
        assert_eq!(button_code(None), Ok(BTN_LEFT));
        assert_eq!(button_code(Some(&"right".to_owned())), Ok(BTN_RIGHT));
        assert_eq!(button_code(Some(&"middle".to_owned())), Ok(BTN_MIDDLE));
        assert!(button_code(Some(&"thumb".to_owned())).is_err());
    }
}
