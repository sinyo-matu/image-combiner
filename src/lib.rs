mod test;

use image::error::ImageError;
use image::{DynamicImage, GenericImage, GenericImageView, ImageBuffer, Rgba};
use imageproc::drawing::{draw_line_segment_mut, draw_text_mut};
use log::debug;
use rusttype::{Font, Scale};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::task::JoinError;
use tokio::{sync::Mutex, task::JoinHandle};

#[derive(Debug)]
pub enum ProcessorError {
    ImageProcessError(ImageError),
    RuntimeError(JoinError),
    InvalidTableError(String),
    InvalidTextError(String),
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

const BLACK_COLOR: Rgba<u8> = image::Rgba([0u8, 0u8, 0u8, 255u8]);
const WHITE_COLOR: Rgba<u8> = image::Rgba([255u8, 255u8, 255u8, 0u8]);
const GRAY_COLOR: Rgba<u8> = image::Rgba([219u8, 219u8, 219u8, 255u8]);

pub struct Processor;

impl Default for Processor {
    fn default() -> Self {
        Self
    }
}

impl Processor {
    pub async fn create_bundled_image_from_bytes(
        &self,
        buffers: Vec<Vec<u8>>,
        options: CreateBundledImageOptions,
    ) -> Result<Vec<u8>, ProcessorError> {
        debug!("process {} images into 1", buffers.len());
        let origin_images = load_images_from_vec(buffers)?;
        let (width, height) = match options.dimension {
            Some(user_setting_dimension) => user_setting_dimension,
            None => find_optical_dimension(&origin_images),
        };
        let resize_images = resize_images(origin_images, width, height).await?;
        let row = (resize_images.len() as f32 / options.column as f32).ceil() as u32;
        let canvas_height = if row >= 1 {
            height + options.padding
        } else {
            height
        };
        let canvas_width = if options.column >= 1 {
            width + options.padding
        } else {
            width
        };

        let bundled_image_canvas_height = row * canvas_height;
        let bundled_image_canvas_width = options.column * canvas_width;
        debug!(
            "create image buf {}x{}",
            bundled_image_canvas_width, bundled_image_canvas_height
        );
        let image_buf = ImageBuffer::from_fn(
            bundled_image_canvas_width,
            bundled_image_canvas_height,
            |_, _| WHITE_COLOR,
        );
        let image_buf_threaded = Arc::new(Mutex::new(image_buf));
        draw_bundled_image(
            Arc::clone(&image_buf_threaded),
            resize_images,
            options.column,
            height,
            canvas_width,
            canvas_height,
            0,
        )
        .await?;
        let dyn_image = DynamicImage::ImageRgba8(image_buf_threaded.lock_owned().await.to_owned());
        let mut image_bytes = Vec::new();
        dyn_image.write_to(&mut image_bytes, image::ImageOutputFormat::Jpeg(100))?;
        Ok(image_bytes)
    }

    pub async fn add_table(
        &self,
        buffer: Vec<u8>,
        table_base: TableBase,
        font_bytes: &'_ [u8],
    ) -> Result<Vec<u8>, ProcessorError> {
        let origin_image = image::load_from_memory(&buffer)?;
        let padding = origin_image.width() as f32 * 0.05;
        let font_size = (origin_image.width() as f32 - padding * 2.0) * 0.03;
        debug!("font size is {}", font_size);
        let cell_padding_x = font_size * 0.75;
        let cell_padding_y = font_size * 0.25;
        let table = table_base.build(cell_padding_x, cell_padding_y, font_size);

        debug!("table width is {}", table.table_width());
        if table.table_width() > origin_image.width() as f32 {
            debug!("table width would be bigger than origin image width return error");
            return Err(ProcessorError::InvalidTableError(format!(
                "table size over table width is {},canvas width is {}",
                table.table_width(),
                origin_image.width()
            )));
        };

        let table_canvas_height = table.table_height().ceil() as u32 + padding as u32 * 2;
        debug!("table height is {}", table.table_height());
        let mut full_canvas = ImageBuffer::from_fn(
            origin_image.width(),
            origin_image.height() + table_canvas_height,
            |_, _| image::Rgba([255, 255, 255, 0] as [u8; 4]),
        );
        debug!(
            "full canvas size is width:{} height: {}",
            origin_image.width(),
            origin_image.height() + table_canvas_height
        );
        //draw table
        {
            let mut table_canvas =
                full_canvas.sub_image(0, 0, origin_image.width(), table_canvas_height);
            let font: Font<'_> = Font::try_from_bytes(font_bytes).unwrap();
            for (top, left, text) in
                table.text_top_left_position(padding, table_canvas.width() as f32, cell_padding_y)
            {
                draw_text_mut(
                    &mut table_canvas,
                    BLACK_COLOR,
                    left.ceil() as u32,
                    top.ceil() as u32,
                    Scale::uniform(font_size),
                    &font,
                    text,
                );
            }
            for (start, end) in table.table_line_position(padding, origin_image.width() as f32) {
                draw_line_segment_mut(&mut table_canvas, start, end, BLACK_COLOR);
            }
        }
        //draw origin image
        {
            debug!("draw origin image");
            let mut origin_canvas = full_canvas.sub_image(
                0,
                table_canvas_height,
                origin_image.width(),
                origin_image.height(),
            );
            origin_canvas.copy_from(&origin_image, 0, 0)?;
        }

        let dyn_image = DynamicImage::ImageRgba8(full_canvas);
        let mut image_bytes = Vec::new();
        dyn_image.write_to(&mut image_bytes, image::ImageOutputFormat::Jpeg(100))?;
        Ok(image_bytes)
    }

    pub async fn create_bundled_image_from_bytes_with_table(
        &self,
        buffers: Vec<Vec<u8>>,
        table_base: TableBase,
        options: CreateBundledImageOptions,
        font_bytes: &'_ [u8],
    ) -> Result<Vec<u8>, ProcessorError> {
        debug!("process {} images into 1", buffers.len());
        let origin_images = load_images_from_vec(buffers)?;
        let (width, height) = match options.dimension {
            Some(user_setting_dimension) => user_setting_dimension,
            None => find_optical_dimension(&origin_images),
        };
        let resize_images = resize_images(origin_images, width, height).await?;
        let row = (resize_images.len() as f32 / options.column as f32).ceil() as u32;
        let canvas_height = if row >= 1 {
            height + options.padding
        } else {
            height
        };
        let canvas_width = if options.column >= 1 {
            width + options.padding
        } else {
            width
        };

        let bundled_image_canvas_height = row * canvas_height;
        let bundled_image_canvas_width = options.column * canvas_width;
        let padding = bundled_image_canvas_width as f32 * 0.05;
        let font_size = (bundled_image_canvas_width as f32 - padding * 2.0) * 0.03;
        debug!("font size is {}", font_size);
        let cell_padding_x = font_size * 0.75;
        let cell_padding_y = font_size * 0.25;
        let table = table_base.build(cell_padding_x, cell_padding_y, font_size);
        let table_canvas_height = table.table_height().ceil() as u32 + padding.ceil() as u32 * 2;
        let table_canvas_width = table.table_width() + padding * 2.0;
        if table_canvas_width.ceil() as u32 > bundled_image_canvas_width {
            debug!("table width would be bigger than origin image width return error");
            return Err(ProcessorError::InvalidTableError(format!(
                "table size over table width is {},canvas width is {}",
                table_canvas_width.ceil() as u32,
                table_canvas_width.ceil()
            )));
        };

        let full_canvas_height = bundled_image_canvas_height + table_canvas_height;
        debug!(
            "create image buf {}x{}",
            bundled_image_canvas_width, full_canvas_height
        );
        let image_buf =
            ImageBuffer::from_fn(bundled_image_canvas_width, full_canvas_height, |_, _| {
                WHITE_COLOR
            });
        let image_buf_threaded = Arc::new(Mutex::new(image_buf));
        draw_bundled_image(
            Arc::clone(&image_buf_threaded),
            resize_images,
            options.column,
            height,
            canvas_width,
            canvas_height,
            table_canvas_height,
        )
        .await?;
        {
            let mut image_buf_lock = image_buf_threaded.lock().await;
            let mut table_canvas =
                image_buf_lock.sub_image(0, 0, bundled_image_canvas_width, table_canvas_height);
            let font: Font<'_> = Font::try_from_bytes(font_bytes).unwrap();
            for (top, left, text) in table.text_top_left_position(
                padding,
                bundled_image_canvas_width as f32,
                cell_padding_y,
            ) {
                draw_text_mut(
                    &mut table_canvas,
                    BLACK_COLOR,
                    left.ceil() as u32,
                    top.ceil() as u32,
                    Scale::uniform(font_size),
                    &font,
                    text,
                );
            }
            for (start, end) in
                table.table_line_position(padding, bundled_image_canvas_width as f32)
            {
                draw_line_segment_mut(&mut table_canvas, start, end, GRAY_COLOR);
            }
        }

        let dyn_image = DynamicImage::ImageRgba8(image_buf_threaded.lock_owned().await.to_owned());
        let mut image_bytes = Vec::new();
        dyn_image.write_to(&mut image_bytes, image::ImageOutputFormat::Jpeg(100))?;
        Ok(image_bytes)
    }

    pub async fn create_bundled_image_from_bytes_with_text<'a>(
        &self,
        buffers: Vec<Vec<u8>>,
        text: &'a str,
        options: CreateBundledImageOptions,
        font_bytes: &'a [u8],
    ) -> Result<Vec<u8>, ProcessorError> {
        debug!("process {} images into 1", buffers.len());
        let origin_images = load_images_from_vec(buffers)?;
        let (width, height) = match options.dimension {
            Some(user_setting_dimension) => user_setting_dimension,
            None => find_optical_dimension(&origin_images),
        };
        let resize_images = resize_images(origin_images, width, height).await?;
        let row = (resize_images.len() as f32 / options.column as f32).ceil() as u32;
        let canvas_height = if row >= 1 {
            height + options.padding
        } else {
            height
        };
        let canvas_width = if options.column >= 1 {
            width + options.padding
        } else {
            width
        };

        let bundled_image_canvas_height = row * canvas_height;
        let bundled_image_canvas_width = options.column * canvas_width;
        let padding = bundled_image_canvas_width as f32 * 0.05;
        let font_size = (bundled_image_canvas_width as f32 - padding * 2.0) * 0.03;
        debug!("font size is {}", font_size);
        let text_canvas_width = calc_chars_len(text) as f32 * font_size + padding * 2.0;
        if text_canvas_width.ceil() as u32 > bundled_image_canvas_width {
            return Err(ProcessorError::InvalidTextError(format!(
                "text canvas width is bigger than image canvas text:{},image:{}",
                text_canvas_width.ceil() as u32,
                bundled_image_canvas_width
            )));
        }
        let text_canvas_height = (font_size + padding * 2.0).ceil() as u32;

        let full_canvas_height = bundled_image_canvas_height + text_canvas_height;
        debug!(
            "create image buf {}x{}",
            bundled_image_canvas_width, full_canvas_height
        );
        let image_buf =
            ImageBuffer::from_fn(bundled_image_canvas_width, full_canvas_height, |_, _| {
                WHITE_COLOR
            });
        let image_buf_threaded = Arc::new(Mutex::new(image_buf));
        draw_bundled_image(
            Arc::clone(&image_buf_threaded),
            resize_images,
            options.column,
            height,
            canvas_width,
            canvas_height,
            text_canvas_height,
        )
        .await?;
        {
            let font: Font<'a> = Font::try_from_bytes(font_bytes).unwrap();
            let mut image_buf_threaded_locked = image_buf_threaded.lock().await;
            let mut text_canvas = image_buf_threaded_locked.sub_image(
                0,
                0,
                bundled_image_canvas_width,
                text_canvas_height,
            );
            draw_text_mut(
                &mut text_canvas,
                BLACK_COLOR,
                padding.ceil() as u32,
                padding.ceil() as u32,
                Scale::uniform(font_size),
                &font,
                text,
            );
        }
        let dyn_image = DynamicImage::ImageRgba8(image_buf_threaded.lock_owned().await.to_owned());
        let mut image_bytes = Vec::new();
        dyn_image.write_to(&mut image_bytes, image::ImageOutputFormat::Jpeg(100))?;
        Ok(image_bytes)
    }

    pub async fn create_table_image(
        &self,
        table_base: TableBase,
        font_bytes: &'_ [u8],
    ) -> Result<Vec<u8>, ProcessorError> {
        let canvas_width = 960u32;

        let padding = canvas_width as f32 * 0.05;
        let font_size = (canvas_width as f32 - padding * 2.0) * 0.03;
        debug!("font size is {}", font_size);
        let cell_padding_x = font_size * 0.75;
        let cell_padding_y = font_size * 0.25;
        let table = table_base.build(cell_padding_x, cell_padding_y, font_size);
        let table_canvas_height = table.table_height().ceil() as u32 + padding.ceil() as u32 * 2;
        let table_canvas_width = table.table_width() + padding * 2.0;

        let mut image_buf = ImageBuffer::from_fn(
            table_canvas_width.ceil() as u32,
            table_canvas_height,
            |_, _| WHITE_COLOR,
        );
        let font: Font<'_> = Font::try_from_bytes(font_bytes).unwrap();
        for (top, left, text) in
            table.text_top_left_position(padding, table_canvas_width.ceil(), cell_padding_y)
        {
            draw_text_mut(
                &mut image_buf,
                BLACK_COLOR,
                left.ceil() as u32,
                top.ceil() as u32,
                Scale::uniform(font_size),
                &font,
                text,
            );
        }
        for (start, end) in table.table_line_position(padding, table_canvas_width.ceil()) {
            draw_line_segment_mut(&mut image_buf, start, end, GRAY_COLOR);
        }

        let dyn_image = DynamicImage::ImageRgba8(image_buf);
        let mut image_bytes = Vec::new();
        dyn_image.write_to(&mut image_bytes, image::ImageOutputFormat::Jpeg(100))?;
        Ok(image_bytes)
    }

    pub async fn create_text_image<'a>(
        &self,
        text: &'a str,
        font_bytes: &'a [u8],
    ) -> Result<Vec<u8>, ProcessorError> {
        let mut canvas_width = 960u32;

        let padding = canvas_width as f32 * 0.05;
        let font_size = (canvas_width as f32 - padding * 2.0) * 0.03;
        debug!("font size is {}", font_size);
        let text_canvas_width = calc_chars_len(text) as f32 * font_size + padding * 2.0;
        if text_canvas_width.ceil() as u32 > canvas_width {
            canvas_width = text_canvas_width.ceil() as u32 + 100;
        }
        let text_canvas_height = (font_size + padding * 2.0).ceil() as u32;
        let mut text_canvas =
            ImageBuffer::from_fn(canvas_width, text_canvas_height, |_, _| WHITE_COLOR);

        let font: Font<'a> = Font::try_from_bytes(font_bytes).unwrap();
        draw_text_mut(
            &mut text_canvas,
            BLACK_COLOR,
            padding.ceil() as u32,
            padding.ceil() as u32,
            Scale::uniform(font_size),
            &font,
            text,
        );

        let dyn_image = DynamicImage::ImageRgba8(text_canvas);
        let mut image_bytes = Vec::new();
        dyn_image.write_to(&mut image_bytes, image::ImageOutputFormat::Jpeg(100))?;
        Ok(image_bytes)
    }
}
#[derive(Clone)]
pub struct TableBase {
    head: Vec<String>,
    body: Vec<Vec<String>>,
    border_width: u32,
}

impl TableBase {
    pub fn new(
        head: Vec<String>,
        body: Vec<Vec<String>>,
        border_width: u32,
    ) -> Result<Self, ProcessorError> {
        for row in body.iter() {
            if row.len() != head.len() {
                debug!("body colum is not equal to head column");
                return Err(ProcessorError::InvalidTableError(format!(
                    "body colum is not equal to head column head:{},body:{}",
                    head.len(),
                    row.len()
                )));
            }
        }
        Ok(Self {
            head,
            body,
            border_width,
        })
    }

    fn build(self, cell_padding_x: f32, cell_padding_y: f32, cell_font_size: f32) -> Table {
        let mut head: Vec<TableCell> = Vec::new();
        let cell_height = cell_padding_y * 2.0 + cell_font_size + self.border_width as f32;
        for (i, column) in self.head.iter().enumerate() {
            let longest_column_len =
                (0..self.body.len()).fold(calc_chars_len(column), |acc, body_row_index| {
                    let body_row_len = calc_chars_len(self.body[body_row_index][i].as_str());
                    if body_row_len > acc {
                        return body_row_len;
                    }
                    acc
                });
            let text_len = cell_font_size * longest_column_len as f32;
            let width = cell_padding_x * 2.0 + self.border_width as f32 + text_len;
            let cell = TableCell::new(width, cell_height, column, cell_font_size);
            head.push(cell);
        }

        //draw table body
        let mut body: Vec<Vec<TableCell>> = Vec::new();
        for row_string in self.body.into_iter() {
            let mut row: Vec<TableCell> = Vec::new();
            for (j, column) in row_string.into_iter().enumerate() {
                let width = head[j].width;
                row.push(TableCell::new(
                    width,
                    cell_height,
                    column.as_str(),
                    cell_font_size,
                ));
            }
            body.push(row);
        }

        Table::new(head, body, self.border_width)
    }
}

pub struct Table {
    head: Vec<TableCell>,
    body: Vec<Vec<TableCell>>,
    border_width: u32,
}

impl Table {
    fn new(head: Vec<TableCell>, body: Vec<Vec<TableCell>>, border_width: u32) -> Self {
        Self {
            head,
            body,
            border_width,
        }
    }

    fn table_width(&self) -> f32 {
        self.head
            .iter()
            .fold(self.border_width as f32, |acc, c| acc + c.width)
    }

    fn table_height(&self) -> f32 {
        let body_height = self
            .body
            .iter()
            .fold(self.border_width as f32, |acc, r| acc + r[0].height);
        self.head[0].height + body_height
    }

    fn text_top_left_position(
        &self,
        padding: f32,
        full_canvas_width: f32,
        cell_padding_y: f32,
    ) -> Vec<(f32, f32, &String)> {
        let mut res = Vec::new();
        let head_text_top = padding + cell_padding_y + self.border_width as f32;
        //handle table head
        let mut current_cell_x = full_canvas_width * 0.5 - self.table_width() * 0.5;
        for cell in self.head.iter() {
            let head_text_left = current_cell_x + cell.width * 0.5 - cell.text_len * 0.5;
            res.push((head_text_top, head_text_left, &cell.text));
            current_cell_x += cell.width;
        }
        //draw table body
        for (i, row) in self.body.iter().enumerate() {
            let body_cell_top = padding + row[0].height + i as f32 * row[0].height;
            let body_text_top = body_cell_top + cell_padding_y + self.border_width as f32;
            let mut current_cell_x = full_canvas_width * 0.5 - self.table_width() * 0.5;
            for cell in row.iter() {
                let body_text_left = current_cell_x + cell.width * 0.5 - cell.text_len * 0.5;
                res.push((body_text_top, body_text_left, &cell.text));
                current_cell_x += cell.width;
            }
        }
        res
    }

    fn table_line_position(
        &self,
        padding: f32,
        full_canvas_width: f32,
    ) -> Vec<((f32, f32), (f32, f32))> {
        let mut res = Vec::new();
        let row_line_end_x =
            (self.border_width - 1) as f32 + full_canvas_width * 0.5 + self.table_width() * 0.5;
        let row_line_start_x = full_canvas_width * 0.5 - self.table_width() * 0.5;
        for border_shift in 0..self.border_width {
            let column_line_start_y = border_shift as f32 + padding;
            let column_line_end_y = border_shift as f32 + padding + self.table_height();
            let table_top_line = (
                (row_line_start_x, column_line_start_y),
                (row_line_end_x, column_line_start_y),
            );
            res.push(table_top_line);
            let table_bottom_line = (
                (row_line_start_x, column_line_end_y),
                (row_line_end_x, column_line_end_y),
            );
            res.push(table_bottom_line);
        }
        let column_line_start_y = padding;
        let column_line_end_y = (self.border_width - 1) as f32 + padding + self.table_height();
        for border_shift in 0..self.border_width {
            let mut current_column_line_x = border_shift as f32 + row_line_start_x;
            for column in self.head.iter() {
                let column_top_line = (
                    (current_column_line_x, column_line_start_y),
                    (current_column_line_x, column_line_end_y),
                );
                res.push(column_top_line);
                current_column_line_x += column.width;
            }
        }

        for border_shift in 0..self.border_width {
            for (i, row) in self.body.iter().enumerate() {
                let row_line_y =
                    border_shift as f32 + column_line_start_y + (i + 1) as f32 * row[0].height;
                let body_row_top_line =
                    ((row_line_start_x, row_line_y), (row_line_end_x, row_line_y));
                res.push(body_row_top_line)
            }
        }
        for border_shift in 0..self.border_width {
            let column_line_x = border_shift as f32 + row_line_end_x;
            let table_right_line = (
                (column_line_x, column_line_start_y),
                (column_line_x, column_line_end_y),
            );

            res.push(table_right_line);
        }
        res
    }
}

pub struct TableCell {
    width: f32,
    height: f32,
    text: String,
    text_len: f32,
}

impl TableCell {
    fn new(width: f32, height: f32, text: &str, font_size: f32) -> Self {
        let chars_count = calc_chars_len(text);
        Self {
            width,
            height,
            text: text.to_owned(),
            text_len: chars_count as f32 * font_size,
        }
    }
}

pub struct CreateBundledImageOptions {
    dimension: Option<(u32, u32)>,
    padding: u32,
    column: u32,
}

impl CreateBundledImageOptions {
    pub fn new(dimension: Option<(u32, u32)>, padding: u32, column: u32) -> Self {
        Self {
            dimension,
            padding,
            column,
        }
    }
}
pub struct CreateBundledImageOptionsBuilder {
    member_dimension: Option<(u32, u32)>,
    column: Option<u32>,
    padding: Option<u32>,
}

impl Default for CreateBundledImageOptionsBuilder {
    fn default() -> Self {
        Self {
            member_dimension: None,
            column: None,
            padding: None,
        }
    }
}

impl CreateBundledImageOptionsBuilder {
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

    pub fn build(&self) -> CreateBundledImageOptions {
        let padding = self.padding.unwrap_or(20);
        let column = self.column.unwrap_or(1);
        CreateBundledImageOptions::new(self.member_dimension, padding, column)
    }
}

fn calc_chars_len(s: &str) -> usize {
    s.chars().fold(0.0, |acc, c| {
        if c.is_ascii() {
            return acc + 0.5;
        }
        acc + 1.0
    }) as usize
}

fn load_images_from_vec(buffers: Vec<Vec<u8>>) -> Result<Vec<DynamicImage>, ProcessorError> {
    let mut origin_images: Vec<DynamicImage> = Vec::new();
    for buf in buffers {
        let origin_image = image::load_from_memory(&buf)?;
        origin_images.push(origin_image);
    }
    Ok(origin_images)
}

async fn resize_images(
    images: Vec<DynamicImage>,
    target_image_width: u32,
    target_image_height: u32,
) -> Result<Vec<DynamicImage>, ProcessorError> {
    let mut resized_images_handles: Vec<JoinHandle<DynamicImage>> = Vec::new();
    for (i, mut origin_image) in images.into_iter().enumerate() {
        let handle = tokio::spawn(async move {
            if origin_image.height() != target_image_height {
                debug!("resize image no {}", i + 1);
                origin_image = origin_image.resize(
                    target_image_width,
                    target_image_height,
                    image::imageops::FilterType::Lanczos3,
                );
            }
            origin_image
        });
        resized_images_handles.push(handle);
    }
    let mut resize_images = Vec::new();
    for handle in resized_images_handles {
        resize_images.push(handle.await?)
    }
    Ok(resize_images)
}

async fn draw_bundled_image(
    image_buf_threaded: Arc<Mutex<ImageBuffer<Rgba<u8>, Vec<u8>>>>,
    images: Vec<DynamicImage>,
    column: u32,
    image_height: u32,
    image_canvas_width: u32,
    image_canvas_height: u32,
    bundled_image_canvas_y: u32,
) -> Result<(), ProcessorError> {
    let mut handles: Vec<JoinHandle<Result<(), ProcessorError>>> = Vec::new();
    for (i, image) in images.into_iter().enumerate() {
        let cloned_image_buf = Arc::clone(&image_buf_threaded);
        let handle = tokio::spawn(async move {
            let current_column = (i as u32 % column) as u32;
            let current_row = (i as u32 / column) as u32;
            debug!("write image no {}", i);
            let image = image.to_rgba8();
            let mut buf = 0;
            if image.height() <= image_height {
                let sub = image_height - image.height();
                buf = sub / 2;
            }
            let mut image_buf = cloned_image_buf.lock().await;
            image_buf.copy_from(
                &image,
                current_column * image_canvas_width,
                current_row * image_canvas_height + buf + bundled_image_canvas_y,
            )?;
            Ok(())
        });
        handles.push(handle)
    }

    for handle in handles {
        handle.await??;
    }
    Ok(())
}

fn find_optical_dimension(origin_images: &[DynamicImage]) -> (u32, u32) {
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

// pub struct AddTableAtTopOptions {
//     column_row_count: Option<(u32, u32)>,
// }

// impl AddTableAtTopOptions {
//     pub fn new(column_row_count: Option<(u32, u32)>) -> Self {
//         Self { column_row_count }
//     }
// }

// pub struct AddTableAtTopOptionsBuilder {
//     column_row_count: Option<(u32, u32)>,
// }

// impl AddTableAtTopOptionsBuilder {
//     pub fn new() -> Self {
//         Self {
//             column_row_count: None,
//         }
//     }

//     pub fn set_column_row_count(mut self, row: u32, column: u32) -> Self {
//         self.column_row_count = Some((row, column));
//         self
//     }

//     pub fn build(&self) -> AddTableAtTopOptions {
//         AddTableAtTopOptions::new(self.column_row_count)
//     }
// }
