use nalgebra::{self, Matrix3, Rotation3};

use nalgebra::{Isometry3, Matrix4, Quaternion, Translation3, UnitQuaternion, Vector3, Vector6};
use ndarray::Axis;
use ndarray::{self, Array2};

use std::ops;

/// A rotation in 3D space.
pub struct Rotation(Rotation3<f32>);

impl ops::Mul<&ndarray::Array2<f32>> for &Rotation {
    type Output = ndarray::Array2<f32>;

    fn mul(self, rhs: &ndarray::Array2<f32>) -> Self::Output {
        let mut result = ndarray::Array2::<f32>::zeros((rhs.len_of(Axis(0)), 3));

        for (in_iter, mut out_iter) in rhs.axis_iter(Axis(0)).zip(result.axis_iter_mut(Axis(0))) {
            let v = self.0 * Vector3::new(in_iter[0], in_iter[1], in_iter[2]);
            out_iter[0] = v[0];
            out_iter[1] = v[1];
            out_iter[2] = v[2];
        }

        result
    }
}

/// A Rigid Body Transform in 3D space.
#[derive(Clone, Debug)]
// #[display(fmt = "Transform: {}", _0)]
pub struct Transform(Isometry3<f32>);

impl Transform {
    /// Create a new transform with zero translation and zero rotation.
    pub fn eye() -> Self {
        Self(Isometry3::<f32>::from_parts(
            Translation3::new(0.0, 0.0, 0.0),
            UnitQuaternion::new(Vector3::<f32>::zeros()),
        ))
    }

    pub fn new(xyz: &Vector3<f32>, rotation: Quaternion<f32>) -> Self {
        Self(Isometry3::<f32>::from_parts(
            Translation3::new(xyz[0], xyz[1], xyz[2]),
            UnitQuaternion::from_quaternion(rotation),
        ))
    }

    /// Create a transform from a 6D vector of the form [x, y, z, rx, ry, rz] where x, y, and z are the translation part
    /// and rx,ry, and rz are the rotation part in the form of a scaled axis.
    ///
    /// # Arguments
    ///
    /// * xyz_so3 - 6D vector of the form [x, y, z, rx, ry, rz]
    ///
    /// # Returns
    ///
    /// * Transform
    pub fn se3_exp(xyz_so3: &Vector6<f32>) -> Self {
        const EPSILON: f32 = 1e-8;

        let omega = Vector3::new(xyz_so3[3], xyz_so3[4], xyz_so3[5]);
        let theta_sq = omega.norm_squared();

        let (theta, quat) = {
            let (theta, imag_factor, real_factor) = if theta_sq < EPSILON * EPSILON {
                let theta_po4 = theta_sq * theta_sq;
                (
                    0.0,
                    0.5 - (1.0 / 48.0) * theta_sq + (1.0 / 3840.0) * theta_po4,
                    1.0 - (1.0 / 8.0) * theta_sq + (1.0 / 384.0) * theta_po4,
                )
            } else {
                let theta = theta_sq.sqrt();
                let half_theta = 0.5 * theta;
                (theta, half_theta.sin() / theta, half_theta.cos())
            };
            (
                theta,
                UnitQuaternion::from_quaternion(nalgebra::Quaternion::new(
                    real_factor,
                    imag_factor * omega[0],
                    imag_factor * omega[1],
                    imag_factor * omega[2],
                )),
            )
        };
        let xyz = {
            let left_jacobian = {
                // https://github.com/strasdat/Sophus/blob/main-1.x/sophus/so3.hpp
                let big_omega = omega.cross_matrix();

                if theta_sq < EPSILON {
                    Matrix3::identity() + (big_omega * 0.5)
                } else {
                    let big_omega_squared = big_omega * big_omega;
                    Matrix3::identity()
                        + (1.0 - theta.cos()) / theta_sq * big_omega
                        + (theta - theta.sin()) / (theta_sq * theta) * big_omega_squared
                }
            };

            left_jacobian * Vector3::new(xyz_so3[0], xyz_so3[1], xyz_so3[2])
        };

        Self(Isometry3::<f32>::from_parts(xyz.into(), quat))
    }

    /// Create a transform from a 4x4 matrix.
    pub fn from_matrix4(matrix: &Matrix4<f32>) -> Self {
        let translation = Translation3::new(matrix[(0, 3)], matrix[(1, 3)], matrix[(2, 3)]);
        let so3 = UnitQuaternion::from_rotation_matrix(&Rotation3::from_matrix(
            &matrix.fixed_slice::<3, 3>(0, 0).into_owned(),
        ));
        Self(Isometry3::<f32>::from_parts(translation, so3))
    }

    /// Transforms a 3D point.
    ///
    /// # Arguments
    ///
    /// * rhs - 3D point.
    ///
    /// # Returns
    ///
    /// * 3D point transformed.
    pub fn transform_vector(&self, rhs: &Vector3<f32>) -> Vector3<f32> {
        self.0.rotation * rhs + self.0.translation.vector
    }

    /// Transforms a 3D normal. That's use only the rotation part of the transform.
    ///
    /// # Arguments
    ///
    /// * rhs - 3D normal.
    ///
    /// # Returns
    ///
    /// * 3D normal transformed.
    pub fn transform_normal(&self, rhs: &Vector3<f32>) -> Vector3<f32> {
        self.0.rotation * rhs
    }

    /// Transforms an array of 3D points.
    ///
    /// # Arguments
    ///
    /// * rhs - Array of 3D points of shape (N, 3).
    ///
    /// # Returns
    ///[[1.4409556, 4.278638, 10.567257]]
    /// * Array of 3D points of shape (N, 3) transformed.
    pub fn transform(&self, mut rhs: Array2<f32>) -> Array2<f32> {
        for mut point in rhs.axis_iter_mut(Axis(0)) {
            let v = self.transform_vector(&Vector3::new(point[0], point[1], point[2]));

            point[0] = v[0];
            point[1] = v[1];
            point[2] = v[2];
        }

        rhs
    }

    /// Returns the rotation part.
    ///
    /// # Returns
    ///
    /// * Rotation
    pub fn ortho_rotation(&self) -> Rotation {
        Rotation(self.0.rotation.to_rotation_matrix())
    }

    /// Inverts the transform.
    pub fn inverse(&self) -> Self {
        Self(self.0.inverse())
    }

    pub fn angle(&self) -> f32 {
        self.0.rotation.angle()
    }

    pub fn translation(&self) -> Vector3<f32> {
        self.0.translation.vector
    }

    pub fn scale_translation(&mut self, scale: f32) {
        self.0.translation.vector *= scale;
    }
}

impl ops::Mul<&ndarray::Array2<f32>> for &Transform {
    type Output = ndarray::Array2<f32>;

    /// Transforms an array of 3D points.
    ///
    /// # Arguments
    ///
    /// * rhs - Array of 3D points of shape (N, 3).
    ///
    /// # Returns
    ///
    /// * Array of 3D points of shape (N, 3) transformed.
    fn mul(self, rhs: &ndarray::Array2<f32>) -> Self::Output {
        let mut result = ndarray::Array2::<f32>::zeros((rhs.len_of(Axis(0)), 3));

        for (in_iter, mut out_iter) in rhs.axis_iter(Axis(0)).zip(result.axis_iter_mut(Axis(0))) {
            let v = self.transform_vector(&Vector3::new(in_iter[0], in_iter[1], in_iter[2]));
            out_iter[0] = v[0];
            out_iter[1] = v[1];
            out_iter[2] = v[2];
        }

        result
    }
}

impl ops::Mul<&Transform> for &Transform {
    type Output = Transform;

    /// Composes two transforms.
    ///
    /// # Arguments
    ///
    /// * rhs - Transform to compose with, i.e. self * rhs, where rhs is applied first.
    ///
    /// # Returns
    ///
    /// * Composed transform.
    fn mul(self, rhs: &Transform) -> Self::Output {
        Transform(self.0 * rhs.0)
    }
}

impl From<&Transform> for Matrix4<f32> {
    /// Converts a transform to a 4x4 matrix.
    fn from(transform: &Transform) -> Self {
        transform.0.into()
    }
}

#[cfg(test)]
mod tests {
    use super::Transform;
    use nalgebra::Vector6;
    use nalgebra::{Isometry3, Matrix4, Translation3, UnitQuaternion, Vector3, Vector4};
    use ndarray::array;

    use ndarray::prelude::*;

    fn assert_array(f1: &Array2<f32>, f2: &Array2<f32>) -> bool {
        if f1.shape() != f2.shape() {
            return false;
        }

        let shape = f1.shape();
        let size = shape[0] * shape[1];
        f1.clone()
            .into_shape((size, 1))
            .iter()
            .zip(f2.clone().into_shape((size, 1)).iter())
            .all(|(v1, v2)| (v1[[0, 0]] - v2[[0, 0]]).abs() < 1e-5)
    }

    #[test]
    fn test_mul_op() {
        let transform = Transform::eye();
        let points = array![[1., 2., 3.], [4., 5., 6.], [7., 8., 9.]];
        let mult_result = &transform * &points;

        assert_eq!(mult_result, points);

        let transform = Transform(Isometry3::from_parts(
            Translation3::<f32>::new(0., 0., 3.),
            UnitQuaternion::<f32>::from_scaled_axis(Vector3::y() * std::f32::consts::PI),
        ));

        assert!(assert_array(
            &(&transform * &array![[1.0, 2.0, 3.0], [1.0, 2.0, 3.0]]),
            &array![[-1.0, 2.0, 0.0], [-1.0, 2.0, 0.0]]
        ));
    }

    #[test]
    fn test_transform() {
        let transform = Transform(Isometry3::from_parts(
            Translation3::<f32>::new(0., 0., 3.),
            UnitQuaternion::<f32>::from_scaled_axis(Vector3::y() * std::f32::consts::PI),
        ));
        let mut points = array![[1.0, 2.0, 3.0], [1.0, 2.0, 3.0]];
        points = transform.transform(points);

        assert!(assert_array(
            &points,
            &array![[-1.0, 2.0, 0.0], [-1.0, 2.0, 0.0]]
        ));
    }

    #[test]
    fn test_exp() {
        let transform = Transform::se3_exp(&Vector6::new(1.0, 2.0, 3.0, 0.4, 0.5, 0.3));

        assert!(assert_array(
            &transform.transform(array![[5.5, 6.4, 7.8]]),
            &array![[8.9848175, 6.9635687, 9.880962]]
        ));

        let se3 = Transform::se3_exp(&Vector6::new(1.0, 2.0, 3.0, 0.4, 0.5, 0.3));
        let matrix = Matrix4::from(&se3);
        let test_mult = matrix * Vector4::new(1.0, 2.0, 3.0, 1.0);
        assert_eq!(
            test_mult,
            Vector4::new(3.5280778, 2.8378963, 5.8994026, 1.0000)
        );
        let test_mult = se3.transform_vector(&Vector3::new(1.0, 2.0, 3.0));
        assert_eq!(
            (test_mult - Vector3::new(3.5280778, 2.8378963, 5.8994026)).norm(),
            0.0
        );
    }

    #[test]
    fn test_compose() {
        let transform1 = Transform(Isometry3::from_parts(
            Translation3::<f32>::new(0., 0., 3.),
            UnitQuaternion::<f32>::identity(),
        ));
        let transform2 = Transform(Isometry3::from_parts(
            Translation3::<f32>::new(0., 0., 3.),
            UnitQuaternion::<f32>::from_scaled_axis(Vector3::y() * std::f32::consts::PI / 2.0),
        ));

        let transform = &transform1 * &transform2;
        assert!(assert_array(
            &transform.transform(array![[1.0, 2.0, 3.0], [1.0, 2.0, 3.0]]),
            &array![[2.9999998, 2.0, 5.0], [2.9999998, 2.0, 5.0]]
        ));
    }
}
