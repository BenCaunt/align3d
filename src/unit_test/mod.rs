mod datasets;
pub use datasets::{sample_rgbd_dataset1, sample_rgbd_frame_dataset1, TestRgbdFrameDataset};
mod geometries;
pub use geometries::sample_teapot_geometry;
mod images;
pub use images::{bloei_luma16, bloei_luma8, bloei_rgb};
mod point_clouds;
pub use point_clouds::{sample_pcl_ds1, sample_teapot_pointcloud, TestPclDataset};
mod range_images;
pub use range_images::{sample_range_img_ds1, sample_range_img_ds2, TestRangeImageDataset};