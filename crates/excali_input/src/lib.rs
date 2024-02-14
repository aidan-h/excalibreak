use winit::dpi::{PhysicalPosition, PhysicalSize};
use winit::event::{DeviceId, Event, MouseButton, WindowEvent};
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

pub struct Input {
    pub mouse_position: Option<MousePosition>,
    pub window_id: WindowId,
    pub left_mouse_click: bool,
    pub right_mouse_click: bool,
    pub middle_mouse_click: bool,
    cursor_device_id: Option<DeviceId>,
}

impl Input {
    pub fn new(window_id: WindowId) -> Self {
        Self {
            mouse_position: None,
            left_mouse_click: false,
            right_mouse_click: false,
            middle_mouse_click: false,
            window_id,
            cursor_device_id: None,
        }
    }

    pub fn clear(&mut self) {
        self.left_mouse_click = false;
        self.right_mouse_click = false;
        self.middle_mouse_click = false;
    }

    pub fn handle_event<T>(&mut self, event: &Event<T>)
    where
        T: 'static,
    {
        if let Event::WindowEvent { window_id, event } = event {
            if *window_id != self.window_id {
                return;
            }
            match event {
                WindowEvent::MouseInput {
                    device_id, button, ..
                } => {
                    if Some(*device_id) == self.cursor_device_id {
                        match button {
                            MouseButton::Left => self.left_mouse_click = true,
                            MouseButton::Right => self.right_mouse_click = true,
                            MouseButton::Middle => self.middle_mouse_click = true,
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
