mod test;

use image::error::ImageError;
use image::{DynamicImage, GenericImageView, ImageBuffer};
use log::debug;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::task::JoinError;
use tokio::{sync::Mutex, task::JoinHandle};

#[derive(Debug)]
pub enum ProcessorError {
    ImageProcessError(ImageError),
    RuntimeError(JoinError),
}

impl From<ImageError> for ProcessorError {
    fn from(e: ImageError) -> Self {
        Self::ImageProcessError(e)
    }
}

impl From<JoinError> for ProcessorError {
    fn from(e: JoinError) -> Self {
        Self::RuntimeError(e)
    }
}

pub struct Processor {
    dimension: Option<(u32, u32)>,
    padding: u32,
    column: u32,
}
impl Processor {
    pub async fn create_bundled_image_from_bytes(
        &self,
        buffers: Vec<Vec<u8>>,
    ) -> Result<Vec<u8>, ProcessorError> {
        debug!("process {} images into 1", buffers.len());
        let mut origin_images: Vec<DynamicImage> = Vec::new();
        for buf in buffers {
            let origin_image = image::load_from_memory(&buf)?;
            origin_images.push(origin_image);
        }
        let (width, height) = match self.dimension {
            Some(user_setting_dimension) => user_setting_dimension,
            None => find_optical_dimension(&origin_images),
        };

        let mut resized_images_handles: Vec<JoinHandle<DynamicImage>> = Vec::new();
        for (i, mut origin_image) in origin_images.into_iter().enumerate() {
            let handle = tokio::spawn(async move {
                if origin_image.height() != height {
                    debug!("resize image no {}", i + 1);
                    origin_image =
                        origin_image.resize(width, height, image::imageops::FilterType::Lanczos3);
                }
                origin_image
            });
            resized_images_handles.push(handle);
        }
        let mut resize_images = Vec::new();
        for handle in resized_images_handles {
            resize_images.push(handle.await?)
        }
        let row = (resize_images.len() as f32 / self.column as f32).ceil() as u32;
        let canvas_height = if row >= 1 {
            height + self.padding
        } else {
            height
        };
        let canvas_width = if self.column >= 1 {
            width + self.padding
        } else {
            width
        };
        let target_height = row * canvas_height;
        let target_width = self.column * canvas_width;
        debug!("create image buf {}x{}", target_width, target_height);
        let image_buf = ImageBuffer::from_fn(target_width, target_height, |_, _| {
            image::Rgba([255, 255, 255, 0] as [u8; 4])
        });
        let image_buf_threaded = Arc::new(Mutex::new(image_buf));
        let mut handles: Vec<JoinHandle<()>> = Vec::new();
        for (i, image) in resize_images.into_iter().enumerate() {
            let cloned_image_buf = Arc::clone(&image_buf_threaded);
            let column = self.column;
            let canvas_width = canvas_width;
            let handle = tokio::spawn(async move {
                let current_column = (i as u32 % column) as u32;
                let current_row = (i as u32 / column) as u32;
                debug!("write image no {}", i);
                let image = image.to_rgba8();
                let mut buf = 0;
                if image.height() <= height {
                    let sub = height - image.height();
                    buf = sub / 2;
                }
                let mut image_buf = cloned_image_buf.lock().await;
                for (x, y, pixel) in image.enumerate_pixels() {
                    let buf_x = x + current_column * canvas_width;
                    let buf_y = y + current_row * canvas_height + buf;
                    let target_pixel = image_buf.get_pixel_mut(buf_x, buf_y);
                    *target_pixel = pixel.to_owned();
                }
            });
            handles.push(handle)
        }

        for handle in handles {
            handle.await?;
        }
        let dyn_image = DynamicImage::ImageRgba8(image_buf_threaded.lock_owned().await.to_owned());
        let mut image_bytes = Vec::new();
        dyn_image.write_to(&mut image_bytes, image::ImageOutputFormat::Jpeg(100))?;
        Ok(image_bytes)
    }
}

pub struct ProcessorBuilder {
    member_dimension: Option<(u32, u32)>,
    column: Option<u32>,
    padding: Option<u32>,
}

impl ProcessorBuilder {
    pub fn new() -> Self {
        Self {
            member_dimension: None,
            column: None,
            padding: None,
        }
    }

    pub fn set_member_dimension(mut self, width: u32, height: u32) -> Self {
        self.member_dimension = Some((width, height));
        self
    }

    pub fn set_column(mut self, column: u32) -> Self {
        self.column = Some(column);
        self
    }

    pub fn set_padding(mut self, padding: u32) -> Self {
        self.padding = Some(padding);
        self
    }

    pub fn build(&self) -> Processor {
        let padding = match self.padding {
            Some(padding) => padding,
            None => 20,
        };
        let column = match self.column {
            Some(column) => column,
            None => 1,
        };
        Processor {
            dimension: self.member_dimension,
            padding,
            column,
        }
    }
}

fn find_optical_dimension(origin_images: &Vec<DynamicImage>) -> (u32, u32) {
    let mut dimension_map: HashMap<(u32, u32), u8> = std::collections::HashMap::new();
    let mut max_dimension = (0, 0);
    let mut max_count = 0;
    let mut most_frequent_dimension = (0, 0);
    for origin_image in origin_images {
        let dimension_sum = origin_image.width() + origin_image.height();
        if dimension_sum > (max_dimension.0 + max_dimension.1) {
            max_dimension = (origin_image.width(), origin_image.height());
            debug!(
                "update max_dimension : width:{}, height:{}",
                max_dimension.0, max_dimension.1
            );
        }
        let count = dimension_map
            .entry((origin_image.width(), origin_image.height()))
            .or_insert(0);
        *count += 1;
        if *count > max_count {
            most_frequent_dimension = (origin_image.width(), origin_image.height());
            max_count = *count;
            debug!(
                "update most_frequent_dimension : width:{}, height:{} count:{}",
                most_frequent_dimension.0, most_frequent_dimension.1, max_count
            );
        }
    }
    if max_count == 1 {
        debug!(
            "max_count is 1 use: width:{} height:{}",
            max_dimension.0, max_dimension.1
        );
        max_dimension
    } else {
        debug!(
            "it's a frequent dimension width:{} height:{} count: {}",
            most_frequent_dimension.0, most_frequent_dimension.1, max_count
        );
        most_frequent_dimension
    }
}
