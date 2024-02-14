use winit::dpi::{PhysicalPosition, PhysicalSize};
use winit::event::{DeviceId, ElementState, Event, MouseButton, WindowEvent};
use winit::window::WindowId;

#[derive(Copy, Clone)]
pub struct MousePosition(PhysicalPosition<f64>);

impl MousePosition {
    pub fn world_position(&self, screen_size: &PhysicalSize<u32>) -> [f32; 2] {
        [
            self.0.x as f32 - screen_size.width as f32 / 2.0,
            -self.0.y as f32 + screen_size.height as f32 / 2.0,
        ]
    }
}

#[derive(Default)]
pub struct Button {
    pub state: InputState,
    /// if last state change was triggered inside UI
    pub consumed: bool,
}

impl Button {
    fn update(&mut self, state: &ElementState, consumed: bool) {
        self.consumed = consumed;
        self.state.update(state);
    }
}

#[derive(Eq, PartialEq)]
pub enum InputState {
    Pressed,
    JustPressed,
    Released,
    JustReleased,
}

impl Default for InputState {
    fn default() -> Self {
        InputState::Released
    }
}

impl InputState {
    fn step(&mut self) {
        match self {
            InputState::JustPressed => *self = InputState::Pressed,
            InputState::JustReleased => *self = InputState::Released,
            _ => {}
        };
    }

    fn update(&mut self, state: &ElementState) {
        *self = match *state {
            ElementState::Pressed => InputState::JustPressed,
            ElementState::Released => InputState::JustReleased,
        };
    }

    pub fn pressed(&self) -> bool {
        matches!(*self, InputState::Pressed | InputState::JustPressed)
    }
}

pub struct Input {
    pub mouse_position: Option<MousePosition>,
    pub window_id: WindowId,
    pub left_mouse_click: Button,
    pub right_mouse_click: Button,
    pub middle_mouse_click: Button,
    cursor_device_id: Option<DeviceId>,
}

impl Input {
    pub fn new(window_id: WindowId) -> Self {
        Self {
            mouse_position: None,
            left_mouse_click: Default::default(),
            right_mouse_click: Default::default(),
            middle_mouse_click: Default::default(),
            window_id,
            cursor_device_id: None,
        }
    }

    pub fn clear(&mut self) {
        self.left_mouse_click.state.step();
        self.right_mouse_click.state.step();
        self.middle_mouse_click.state.step();
    }

    pub fn handle_event<T>(&mut self, event: &Event<T>, consumed: bool)
    where
        T: 'static,
    {
        if let Event::WindowEvent { window_id, event } = event {
            if *window_id != self.window_id {
                return;
            }
            match event {
                WindowEvent::MouseInput {
                    device_id,
                    button,
                    state,
                    ..
                } => {
                    if Some(*device_id) == self.cursor_device_id {
                        match button {
                            MouseButton::Left => self.left_mouse_click.update(state, consumed),
                            MouseButton::Right => self.right_mouse_click.update(state, consumed),
                            MouseButton::Middle => self.middle_mouse_click.update(state, consumed),
                            _ => {}
                        }
                    }
                }
                WindowEvent::CursorLeft { device_id } => {
                    if Some(*device_id) == self.cursor_device_id {
                        self.cursor_device_id = None;
                        self.mouse_position = None;
                    }
                }
                WindowEvent::CursorEntered { device_id } => {
                    self.cursor_device_id = Some(*device_id);
                }
                WindowEvent::CursorMoved {
                    device_id,
                    position,
                    ..
                } => {
                    if let Some(cursor_device_id) = self.cursor_device_id {
                        if cursor_device_id == *device_id {
                            self.mouse_position = Some(MousePosition(*position));
                        }
                    }
                }
                _ => {}
            }
        }
    }
}
