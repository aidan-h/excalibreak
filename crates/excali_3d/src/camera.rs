use nalgebra::{Matrix4, Perspective3, Point3, Unit, Vector2, Vector3, Vector4};
use std::f32::consts::PI;

pub trait CameraEye {
    fn target(&self, position: &Point3<f32>) -> Point3<f32>;
}

pub struct FPSEye {
    pub pitch: f32,
    pub yaw: f32,
}

impl Default for FPSEye {
    fn default() -> Self {
        Self {
            pitch: 0.0,
            yaw: PI / 2.0,
        }
    }
}

impl CameraEye for FPSEye {
    fn target(&self, position: &Point3<f32>) -> Point3<f32> {
        let (sin_pitch, cos_pitch) = self.pitch.sin_cos();
        let (sin_yaw, cos_yaw) = self.yaw.sin_cos();
        Vector3::new(
            cos_pitch * cos_yaw + position.x,
            sin_pitch + position.y,
            cos_pitch * sin_yaw + position.z,
        )
        .into()
    }
}

#[derive(Default)]
pub struct LookAtEye {
    pub target: Point3<f32>,
}

impl CameraEye for LookAtEye {
    fn target(&self, _position: &Point3<f32>) -> Point3<f32> {
        self.target
    }
}

pub struct Camera<T: CameraEye> {
    pub eye: T,
    pub up: Vector3<f32>,
    pub aspect: f32,
    pub fovy: f32,
    pub znear: f32,
    pub zfar: f32,
    pub position: Point3<f32>,
}

impl<T: CameraEye + Default> Default for Camera<T> {
    fn default() -> Self {
        Self {
            eye: T::default(),
            up: Vector3::y(),
            aspect: 1.0,
            fovy: 45.0,
            znear: 0.1,
            zfar: 100.0,
            position: Point3::default(),
        }
    }
}

impl Camera<FPSEye> {
    pub fn rotate(&mut self, mouse_delta: &Vector2<f32>, sensitivity: f32) {
        self.eye.yaw += mouse_delta.x * sensitivity;
        const PITCH_LIMIT: f32 = 1.3;
        self.eye.pitch =
            (self.eye.pitch - mouse_delta.y * sensitivity).clamp(-PITCH_LIMIT, PITCH_LIMIT);
    }

    pub fn free_fly(
        &mut self,
        mut direction: Vector3<f32>,
        mouse_delta: &Vector2<f32>,
        distance: f32,
        sensitivity: f32,
    ) {
        self.rotate(mouse_delta, sensitivity);
        if direction.magnitude_squared() > 0.1 {
            direction = direction.normalize() * distance;
            self.position = self.point_to_world_space(&direction).into();
        }
    }
}

impl<T: CameraEye> Camera<T> {
    pub fn point_to_world_space(&self, position: &Vector3<f32>) -> Vector3<f32> {
        (self.model_matrix() * Vector4::new(position.x, position.y, position.z, 1.0)).xyz()
    }

    pub fn projection_matrix(&self) -> Matrix4<f32> {
        Matrix4::<f32>::new_perspective(self.aspect, self.fovy, self.znear, self.zfar) * self.view()
    }

    pub fn view(&self) -> Matrix4<f32> {
        Matrix4::look_at_rh(&self.position, &self.eye.target(&self.position), &self.up)
    }

    pub fn model_matrix(&self) -> Matrix4<f32> {
        Matrix4::face_towards(&self.position, &self.eye.target(&self.position), &self.up)
    }

    #[cfg(feature = "parry3d")]
    /// mouse_position: clip space of mouse (-1.0 -> 1.0)
    pub fn get_ray(&self, mouse_position: Vector2<f32>) -> parry3d::query::Ray {
        //println!("({}, {})", point[0], point[1]);

        let projection = Perspective3::new(self.aspect, self.fovy, self.znear, self.zfar);

        // Compute two points in clip-space.
        // "ndc" = normalized device coordinates.
        let near_ndc_point = Point3::new(mouse_position.x, mouse_position.y, -1.0);
        let far_ndc_point = Point3::new(mouse_position.x, mouse_position.y, 1.0);

        // Unproject them to view-space.
        let near_view_point = projection.unproject_point(&near_ndc_point);
        let far_view_point = projection.unproject_point(&far_ndc_point);

        // Compute the view-space line parameters.
        let inverse = self.view().try_inverse().unwrap();
        let start = (inverse
            * Vector4::new(near_view_point.x, near_view_point.y, near_view_point.z, 1.0))
        .xyz();
        let line_direction = Unit::new_normalize(far_view_point - near_view_point).xyz();
        let direction = (inverse
            * Vector4::new(line_direction.x, line_direction.y, line_direction.z, 1.0))
        .xyz()
            - start;

        parry3d::query::Ray::new(start.into(), direction)
    }
}
