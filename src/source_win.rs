use std::thread;
use std::rc::Rc;
use std::cell::RefCell;
use clock_ticks::precise_time_ns;
use carboxyl::{ Signal, Sink, Stream };
use input::Input;
use button::{ ButtonEvent, ButtonState };
use window;
use ::{ StreamingWindow, RunnableWindow };


/// A wrapper for all event sinks required for implementation
struct EventSinks {
    window_position: Sink<(i32, i32)>,
    window_size: Sink<(u32, u32)>,
    button: Sink<ButtonEvent>,
    mouse_motion: Sink<(f64, f64)>,
    mouse_wheel: Sink<(f64, f64)>,
    focus: Sink<bool>,
    text: Sink<String>,
}

impl EventSinks {
    fn dispatch(&self, event: Input) {
        use input::Motion::*;
        use input::Input::*;

        match event {
            Press(button) =>
                self.button.send(ButtonEvent {
                    button: button,
                    state: ButtonState::Pressed,
                }),
            Release(button) =>
                self.button.send(ButtonEvent {
                    button: button,
                    state: ButtonState::Released,
                }),
            Move(MouseCursor(x, y)) => self.mouse_motion.send((x, y)),
            Move(MouseScroll(x, y)) => self.mouse_wheel.send((x, y)),
            Move(_) => (),
            Text(s) => self.text.send(s),
            Resize(width, height) => self.window_size.send((width, height)),
            Focus(flag) => self.focus.send(flag),
        }
    }
}


/// A source of window events.
///
/// Normally this will be a window handle. This has a minimal API to allow for
/// any kind of other object, too. It is hard-coded to work with piston input
/// events, as it should be easy to convert to those for a given backend.
pub trait EventSource {
    /// Should the window close?
    fn should_close(&self) -> bool;

    /// Poll an event if available (non-blocking).
    fn poll_event(&mut self) -> Option<Input>;
}

impl<W: window::Window<Event=Input>> EventSource for Rc<RefCell<W>> {
    fn should_close(&self) -> bool {
        self.borrow().should_close()
    }

    fn poll_event(&mut self) -> Option<Input> {
        self.borrow_mut().poll_event()
    }
}

impl<W: window::Window<Event=Input>> EventSource for Arc<RwLock<W>> {
    fn should_close(&self) -> bool {
        self.read().unwrap().should_close()
    }

    fn poll_event(&mut self) -> Option<Input> {
        self.write().unwrap().poll_event()
    }
}

impl<W: window::Window<Event=Input>> EventSource for Arc<Mutex<W>> {
    fn should_close(&self) -> bool {
        self.lock().unwrap().should_close()
    }

    fn poll_event(&mut self) -> Option<Input> {
        self.lock().unwrap().poll_event()
    }
}


/// A reactive window implementation generic over the event source.
pub struct SourceWindow<S> {
    source: S,
    sinks: EventSinks,
}

impl<S: EventSource> SourceWindow<S> {
    /// Create a new Glium loop.
    ///
    /// # Parameters
    ///
    /// `tick_length` is the minimum duration of a tick in nanoseconds.
    pub fn new(source: S) -> SourceWindow<S> {
        SourceWindow {
            source: source,
            sinks: EventSinks {
                button: Sink::new(),
                mouse_motion: Sink::new(),
                mouse_wheel: Sink::new(),
                focus: Sink::new(),
                window_position: Sink::new(),
                window_size: Sink::new(),
                text: Sink::new(),
            }
        }
    }
}

impl<S: EventSource> RunnableWindow for SourceWindow<S> {
    fn run_with<F: FnMut()>(&mut self, fps: f64, mut render: F) {
        assert!(fps > 0.0);
        let tick_length = (1e9 / fps) as u64;
        let mut time = precise_time_ns();
        let mut next_tick = time;
        while !self.source.should_close() {
            time = precise_time_ns();
            if time >= next_tick {
                let diff = time - next_tick;
                let delta = diff - diff % tick_length;
                next_tick += delta;
                while let Some(event) = self.source.poll_event() {
                    let _ = self.sinks.dispatch(event);
                }
                render();
            }
            else {
                thread::sleep_ms((next_tick - time) as u32);
            }
        }
    }
}

impl<S> StreamingWindow for SourceWindow<S> {
    fn position(&self) -> Signal<(i32, i32)> {
        self.sinks.window_position.stream().hold((0, 0))
    }

    fn size(&self) -> Signal<(u32, u32)> {
        self.sinks.window_size.stream().hold((0, 0))
    }

    fn buttons(&self) -> Stream<ButtonEvent> {
        self.sinks.button.stream()
    }

    fn text(&self) -> Stream<String> {
        self.sinks.text.stream()
    }

    fn cursor(&self) -> Signal<(f64, f64)> {
        self.sinks.mouse_motion.stream().hold((0.0, 0.0))
    }

    fn wheel(&self) -> Signal<(f64, f64)> {
        self.sinks.mouse_wheel.stream()
            .scan((0.0, 0.0), |(x, y), (dx, dy)| (x + dx, y + dy))
    }

    fn focus(&self) -> Signal<bool> {
        self.sinks.focus.stream().hold(true)
    }
}