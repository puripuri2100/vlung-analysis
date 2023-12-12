use crate::Data;
use image::{Rgb, RgbImage};
use tokio_stream::StreamExt;

pub async fn data_to_img(w: u32, h: u32, data_lst: &[Vec<Data>]) -> RgbImage {
  let mut img = RgbImage::new(w, h);
  for (i, data) in data_lst.iter().enumerate() {
    let color = if i == 0 {
      Rgb([0, 255, 0])
    } else if i == 1 {
      Rgb([0, 0, 255])
    } else if i == 2 {
      Rgb([0, 255, 255])
    } else if i == 3 {
      Rgb([255, 0, 255])
    } else if i == 4 {
      Rgb([255, 255, 0])
    } else {
      Rgb([255, 0, 0])
    };
    let mut data = tokio_stream::iter(data);
    while let Some(d) = data.next().await {
      img.put_pixel(d.point.x as u32, d.point.y as u32, color);
    }
  }
  img
}
