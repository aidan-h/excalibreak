use nalgebra::{Matrix4, UnitQuaternion, Vector3};

pub struct Transform {
    pub position: Vector3<f32>,
    pub scale: Vector3<f32>,
    pub rotation: UnitQuaternion<f32>,
}

impl Transform {
    pub fn matrix(&self) -> Matrix4<f32> {
        Matrix4::new_translation(&self.position)
            * (self.rotation.to_homogeneous() * Matrix4::new_nonuniform_scaling(&self.scale))
    }

    /// Creates a transfrom between two points, forming a line across the z-axis
    pub fn line(a: &Vector3<f32>, b: &Vector3<f32>, thickness: f32) -> Self {
        let direction = b - a;
        let middle = (a + b) / 2.0;
        let length = direction.magnitude();
        Self {
            position: middle,
            scale: Vector3::new(thickness, thickness, length),
            rotation: UnitQuaternion::face_towards(&direction, &Vector3::y()),
        }
    }
}

impl Default for Transform {
    fn default() -> Self {
        Self {
            position: Vector3::zeros(),
            rotation: UnitQuaternion::identity(),
            scale: Vector3::new(1.0, 1.0, 1.0),
        }
    }
}
