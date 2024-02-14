use excali_input::*;
use winit::event::VirtualKeyCode;

pub struct Actions {
    pub undo: Action,
    pub debug: Action,
    pub edit: Action,
    pub escape: Action,
    pub camera_forward: Action,
    pub camera_backward: Action,
    pub camera_left: Action,
    pub camera_right: Action,
    pub camera_up: Action,
    pub camera_down: Action,
}

// TODO wrap both traits into a derive macro
impl Default for Actions {
    fn default() -> Self {
        Self {
            undo: Action::new(VirtualKeyCode::U),
            escape: Action::new(VirtualKeyCode::Escape),
            debug: Action::new(VirtualKeyCode::F2),
            edit: Action::new(VirtualKeyCode::F1),
            camera_forward: Action::new(VirtualKeyCode::W),
            camera_backward: Action::new(VirtualKeyCode::S),
            camera_left: Action::new(VirtualKeyCode::A),
            camera_right: Action::new(VirtualKeyCode::D),
            camera_up: Action::new(VirtualKeyCode::Space),
            camera_down: Action::new(VirtualKeyCode::LShift),
        }
    }
}

impl InputMap for Actions {
    fn actions(&mut self) -> Vec<&mut Action> {
        vec![
            &mut self.undo,
            &mut self.camera_forward,
            &mut self.camera_up,
            &mut self.camera_down,
            &mut self.camera_right,
            &mut self.camera_left,
            &mut self.camera_backward,
            &mut self.debug,
            &mut self.escape,
            &mut self.edit,
        ]
    }
}
