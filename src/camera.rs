use nalgebra::Vector3;

use super::transform::Transform;

/// Camera intrinsic parameters.
#[derive(Clone, Debug)]
pub struct Camera {
    /// Focal length and pixel scale in the X-axis.
    pub fx: f64,
    /// Focal length and pixel scale in the Y-axis.
    pub fy: f64,
    /// Camera X-center.
    pub cx: f64,
    /// Camera Y-center.
    pub cy: f64,
    pub camera_to_world: Option<Transform>,
}

pub struct CameraBuilder(Camera);

impl CameraBuilder {
    /// Creates a camera using the focal length with pixel
    /// scales (fx, fy) and camera center (cx, cy).
    pub fn from_simple_intrinsic(fx: f64, fy: f64, cx: f64, cy: f64) -> Self {
        Self(Camera {
            fx,
            fy,
            cx,
            cy,
            camera_to_world: None,
        })
    }

    pub fn camera_to_world(&'_ mut self, value: Option<Transform>) -> &'_ mut CameraBuilder {
        self.0.camera_to_world = value;
        self
    }

    pub fn build(&self) -> Camera {
        self.0.clone()
    }
}

pub enum PointSpace {
    Camera(Vector3<f32>),
    World(Vector3<f32>),
}

impl Camera {
    /// Project a 3D point into image space.
    ///
    /// # Arguments
    ///
    /// * point: The 3D point.
    ///
    /// # Returns
    ///
    /// * (x and y) coordinates.
    pub fn project(&self, point: &Vector3<f32>) -> (f32, f32) {
        (
            point[0] * self.fx as f32 / point[2] + self.cx as f32,
            point[1] * self.fy as f32 / point[2] + self.cy as f32,
        )
    }

    pub fn project_point(&self, point: &PointSpace) -> Option<(f32, f32)> {
        match (self.camera_to_world.as_ref(), point) {
            (Some(extrinsics), &PointSpace::World(point)) => {
                Some(self.project(&extrinsics.transform_vector(&point)))
            }
            (_, &PointSpace::Camera(point)) => Some(self.project(&point)),
            _ => None,
        }
    }

    pub fn project_grad(&self, point: &Vector3<f32>) -> ((f32, f32), (f32, f32)) {
        let z = point[2];
        let zz = z * z;
        (
            (self.fx as f32 / z, -point[0] * self.fx as f32 / zz),
            (self.fy as f32 / z, -point[1] * self.fy as f32 / zz),
        )
    }

    pub fn backproject(&self, x: f32, y: f32, z: f32) -> Vector3<f32> {
        Vector3::new(
            (x - self.cx as f32) * z / self.fx as f32,
            (y - self.cy as f32) * z / self.fy as f32,
            z,
        )
    }

    /// Scale the camera parameters according to the given scale.
    ///
    /// # Arguments
    ///
    /// * scale: The scale factor.
    ///
    /// # Returns
    ///
    /// * A new camera with scaled parameters.
    pub fn scale(&self, scale: f64) -> Self {
        Self {
            fx: self.fx * scale,
            fy: self.fy * scale,
            cx: self.cx * scale,
            cy: self.cy * scale,
            camera_to_world: None,
        }
    }
}
