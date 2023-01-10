use super::camera::Camera;
use super::io::rgbdimage::RGBDImage;


use nalgebra::Vector3;
use ndarray::iter::AxisIter;
use ndarray::{ArcArray2, Array2, Array3, ArrayView2, Axis};

use crate::io::Geometry;
use crate::pointcloud::PointCloud;

pub struct ImagePointCloud {
    pub points: Array3<f32>,
    pub mask: Array2<u8>,
    pub normals: Option<Array3<f32>>,
    pub colors: Option<Array3<u8>>,
    valid_points: usize,
}

impl ImagePointCloud {
    pub fn from_rgbd_image(camera: &Camera, rgbd_image: RGBDImage) -> Self {
        // TODO produce a warning or return an error

        let (width, height) = (rgbd_image.width(), rgbd_image.height());
        let depth_scale = rgbd_image.depth_scale.unwrap_or(1.0 / 5000.0) as f32;
        let mut points = Array3::zeros((height, width, 3));
        let mut mask = Array2::<u8>::zeros((height, width));
        let mut colors = Array3::<u8>::zeros((height, width, 3));
        let mut valid_points = 0;

        for x in 0..width {
            for y in 0..height {
                let z = rgbd_image.depth[[y, x]];
                if z > 0 {
                    let z = rgbd_image.depth[[y, x]] as f32 * depth_scale;
                    let point3d = camera.backproject(x as f32, y as f32, z);
                    points[[y, x, 0]] = point3d[0];
                    points[[y, x, 1]] = point3d[1];
                    points[[y, x, 2]] = point3d[2];
                    mask[[y, x]] = 1;
                    valid_points += 1;
                }

                colors[[y, x, 0]] = rgbd_image.color[[0, y, x]];
                colors[[y, x, 1]] = rgbd_image.color[[1, y, x]];
                colors[[y, x, 2]] = rgbd_image.color[[2, y, x]];
            }
        }

        Self {
            points,
            mask,
            normals: None,
            colors: Some(colors),
            valid_points,
        }
    }

    pub fn width(&self) -> usize {
        self.points.shape()[1]
    }

    pub fn height(&self) -> usize {
        self.points.shape()[0]
    }

    pub fn valid_points_count(&self) -> usize {
        self.valid_points
    }

    pub fn get_point(&self, row: usize, col: usize) -> Option<nalgebra::Vector3<f32>> {
        if col < self.width() && row < self.height() && self.mask[(row as usize, col as usize)] == 1
        {
            Some(Vector3::new(
                self.points[(row, col, 0)],
                self.points[(row, col, 1)],
                self.points[(row, col, 2)],
            ))
        } else {
            None
        }
    }

    pub fn compute_normals(&mut self) {
        let height = self.height();
        let width = self.width();

        let ratio_threshold = 2f32;
        let ratio_threshold_squared = ratio_threshold * ratio_threshold;

        let mut normals = Array3::<f32>::zeros((height, width, 3));

        for row in 0..height {
            for col in 0..width {
                if self.mask[(row, col)] != 1 {
                    continue;
                };

                let center = nalgebra::Vector3::<f32>::new(
                    self.points[(row, col, 0)],
                    self.points[(row, col, 1)],
                    self.points[(row, col, 2)],
                );
                let left = self
                    .get_point(row, (col as i32 - 1) as usize)
                    .unwrap_or_else(nalgebra::Vector3::<f32>::zeros);
                let right = self
                    .get_point(row, col + 1)
                    .unwrap_or_else(nalgebra::Vector3::<f32>::zeros);

                let left_dist_squared = (left - center).norm_squared();
                let right_dist_squared = (right - center).norm_squared();
                let left_right_ratio = left_dist_squared / right_dist_squared;

                let left_to_right = if left_right_ratio < ratio_threshold_squared
                    && left_right_ratio > 1f32 / ratio_threshold_squared
                {
                    right - left
                } else if left_dist_squared < right_dist_squared {
                    center - left
                } else {
                    right - center
                };

                let bottom = self
                    .get_point(row + 1, col)
                    .unwrap_or_else(nalgebra::Vector3::<f32>::zeros);
                let top = self
                    .get_point((row as i32 - 1) as usize, col)
                    .unwrap_or_else(nalgebra::Vector3::<f32>::zeros);

                let bottom_dist_squared = (bottom - center).norm_squared();
                let top_dist_squared = (top - center).norm_squared();
                let bottom_top_ratio = bottom_dist_squared / top_dist_squared;

                let bottom_to_top = if bottom_top_ratio < ratio_threshold_squared
                    && bottom_top_ratio > 1f32 / ratio_threshold_squared
                {
                    top - bottom
                } else if bottom_dist_squared < top_dist_squared {
                    center - bottom
                } else {
                    top - center
                };

                let normal = left_to_right.cross(&bottom_to_top);

                let normal_magnitude = normal.magnitude();
                if normal_magnitude > 1e-6_f32 {
                    normals[(row, col, 0)] = normal[0] / normal_magnitude;
                    normals[(row, col, 1)] = normal[1] / normal_magnitude;
                    normals[(row, col, 2)] = normal[2] / normal_magnitude;
                }
            }
        }

        self.normals = Some(normals);
    }
}

pub struct PointView<'a> {
    points: ArrayView2<'a, f32>,
    mask: ArrayView2<'a, u8>,
}

pub struct PointViewIterator<'a> {
    iter: std::iter::Zip<
        AxisIter<'a, f32, ndarray::Dim<[usize; 1]>>,
        AxisIter<'a, u8, ndarray::Dim<[usize; 1]>>,
    >,
}

impl<'a> PointView<'a> {
    pub fn iter(&'a self) -> PointViewIterator<'a> {
        PointViewIterator {
            iter: self
                .points
                .axis_iter(Axis(0))
                .zip(self.mask.axis_iter(Axis(0))),
        }
    }
}

impl<'a> Iterator for PointViewIterator<'a> {
    type Item = Vector3<f32>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let (v, m) = self.iter.next()?;
            if m[0] > 0 {
                return Some(Vector3::new(v[0], v[1], v[2]));
            };
        }
    }
}

impl ImagePointCloud {
    pub fn point_view<'a>(&'a self) -> PointView<'a> {
        let total_points = self.width() * self.height();
        let points = self.points.view().into_shape((total_points, 3)).unwrap();
        let mask = self.mask.view().into_shape((total_points, 1)).unwrap();
        PointView {
            points: points,
            mask: mask,
        }
    }
}

impl From<&ImagePointCloud> for PointCloud {
    fn from(image_pcl: &ImagePointCloud) -> PointCloud {
        let num_total_points = image_pcl.width() * image_pcl.height();

        let mask = image_pcl
            .mask
            .view()
            .into_shape((num_total_points,))
            .unwrap();
        let num_valid_points = mask.iter().map(|x| *x as usize).sum();

        // TODO: Improve mask and make a generic function/macro.
        let v: Vec<f32> = image_pcl
            .points
            .view()
            .into_shape((num_total_points, 3))
            .unwrap()
            .axis_iter(Axis(0))
            .enumerate()
            .filter(|(idx, _)| mask[*idx] != 0)
            .flat_map(|(_, v)| [v[0], v[1], v[2]])
            .collect();
        let points = Array2::from_shape_vec((num_valid_points, 3), v).unwrap();

        let normals = image_pcl.normals.as_ref().map(|normals| {
            Array2::from_shape_vec(
                (num_valid_points, 3),
                normals
                    .view()
                    .into_shape((num_total_points, 3))
                    .unwrap()
                    .axis_iter(Axis(0))
                    .enumerate()
                    .filter(|(idx, _)| mask[*idx] != 0)
                    .flat_map(|(_, v)| [v[0], v[1], v[2]])
                    .collect(),
            )
            .unwrap()
        });

        let colors = image_pcl.colors.as_ref().map(|colors| {
            ArcArray2::from_shape_vec(
                (num_valid_points, 3),
                colors
                    .view()
                    .into_shape((num_total_points, 3))
                    .unwrap()
                    .axis_iter(Axis(0))
                    .enumerate()
                    .filter(|(idx, _)| mask[*idx] != 0)
                    .flat_map(|(_, v)| [v[0], v[1], v[2]])
                    .collect(),
            )
            .unwrap()
        });

        PointCloud {
            points,
            normals,
            colors,
        }
    }
}

impl From<&ImagePointCloud> for Geometry {
    fn from(image_pcl: &ImagePointCloud) -> Geometry {
        let pcl = PointCloud::from(image_pcl);
        pcl.into()
    }
}



#[cfg(test)]
mod tests {
    use super::*;
    use crate::io::{dataset::RGBDDataset, slamtb::SlamTbDataset, write_ply};
    use rstest::*;

    #[fixture]
    fn sample1() -> SlamTbDataset {
        SlamTbDataset::load("tests/data/rgbd/sample1").unwrap()
    }

    #[rstest]
    fn should_backproject_rgbd_image(sample1: SlamTbDataset) {
        use crate::io::write_ply;

        let (cam, rgbd_image) = sample1.get_item(0).unwrap();
        let im_pcl = ImagePointCloud::from_rgbd_image(&cam, rgbd_image);

        assert_eq!(480, im_pcl.height());
        assert_eq!(640, im_pcl.width());

        write_ply("tests/data/out-backproj.ply", &Geometry::from(&im_pcl))
            .expect("Error while writing results");
    }

    #[rstest]
    fn should_compute_normals(sample1: SlamTbDataset) {
        let (cam, rgbd_image) = sample1.get_item(0).unwrap();

        let mut im_pcl = ImagePointCloud::from_rgbd_image(&cam, rgbd_image);
        im_pcl.compute_normals();

        {
            let normals = im_pcl.normals.as_ref().unwrap();
            assert_eq!(480, normals.shape()[0]);
            assert_eq!(640, normals.shape()[1]);
        }

        write_ply(
            "tests/data/out-imagepcl-normals.ply",
            &Geometry::from(&im_pcl),
        )
        .expect("Error while writing the results");
    }

    #[rstest]
    fn should_convert_into_pointcloud(sample1: SlamTbDataset) {
        let (cam, rgbd_image) = sample1.get_item(0).unwrap();
        let im_pcl = ImagePointCloud::from_rgbd_image(&cam, rgbd_image);

        let pcl = PointCloud::from(&im_pcl);
        assert_eq!(pcl.len(), 270213);
    }
}
