use std::f32::consts::PI;
use glam::{EulerRot, Mat4, Quat, Vec3};

#[derive(Default, Clone, Debug)]
pub struct Camera {
    position: Vec3,
    rotation: Quat,
    matrix: Option<Mat4>,
}

impl Camera {
    pub fn new() -> Self {
        Self {
            position: Default::default(),
            rotation: Default::default(),
            matrix: None,
        }
    }

    pub fn matrix(&mut self) -> Mat4 {
        if let Some(mat) = self.matrix {
            mat
        } else {
            let pos_mat = Mat4::from_rotation_translation(Quat::default(), -self.position);

            let rot_mat = Mat4::from_rotation_translation(Quat::from_euler(EulerRot::ZXY, PI / 2f32, 0f32, PI / 2f32) * self.rotation, Vec3::default());

            self.matrix = Some(rot_mat * pos_mat);
            self.matrix.unwrap()
        }
    }

    pub fn set_position(&mut self, pos: Vec3) -> &mut Self {
        self.position = pos;
        self.matrix = None;
        self
    }

    pub fn set_rotation(&mut self, rot: Quat) -> &mut Self {
        self.rotation = rot;
        self.matrix = None;
        self
    }

    pub fn set_rotation_euler(&mut self, x: f32, y: f32, z: f32) -> &mut Self {
        self.set_rotation(Quat::from_euler(EulerRot::XYZ, x, y, z));
        self
    }
    pub fn rotation(&self) -> Quat {
        self.rotation
    }
    pub fn position(&self) -> Vec3 {
        self.position
    }
    pub fn euler(&self) -> (f32, f32, f32) {
        self.rotation.to_euler(EulerRot::XYX)
    }
}