use anyhow::{anyhow, Context, Result};
use clap::Parser;
use dicom::object::{open_file, Tag};
use dicom_pixeldata::PixelDecoder;
use regex::Regex;
use tokio::fs;
use tracing::*;

mod filter;
mod k_means;
mod write_image;

#[derive(Parser)]
#[command(author, version)]
struct Args {
  #[arg(short, long)]
  folder: String,
}

async fn init_logger() -> Result<()> {
  let subscriber = tracing_subscriber::fmt()
    .with_max_level(tracing::Level::INFO)
    .finish();
  tracing::subscriber::set_global_default(subscriber)?;
  Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Point {
  pub x: u16,
  pub y: u16,
  pub z: u16,
}

impl Point {
  fn new(x: u16, y: u16, z: u16) -> Self {
    Point { x, y, z }
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Data {
  pub point: Point,
  pub data: i16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Center {
  pub point: Option<Point>,
  pub data: i16,
}

// [WIP]
fn calc_distance(center: &Center, data: &Data) -> usize {
  (center.data as usize).abs_diff(data.data as usize)
}

// [WIP]
fn calc_center(lst: &[Data]) -> Option<Center> {
  let len = lst.len();
  if len == 0 {
    None
  } else {
    let d = (lst.iter().map(|d| d.data as i64).sum::<i64>() / len as i64) as i16;
    Some(Center {
      point: None,
      data: d,
    })
  }
}

// 重心の近さが閾値以下になったら同じと見なす
// [WIP]
fn calc_eq(lst1: &[Data], lst2: &[Data]) -> bool {
  let center_1 = calc_center(lst1);
  let center_2 = calc_center(lst2);
  match (center_1, center_2) {
    (Some(d1), Some(d2)) => d1.data.abs_diff(d2.data) == 0,
    (None, None) => true,
    _ => false,
  }
}

#[tokio::main]
async fn main() -> Result<()> {
  let args = Args::parse();

  init_logger().await?;

  let mut data_lst = Vec::new();
  let depth_re = Regex::new(r"IMG(?<z>\d+)").unwrap();
  let mut rows = 0;
  let mut columns = 0;
  let mut files = fs::read_dir(args.folder).await?;
  while let Some(file) = files.next_entry().await? {
    let filename = file.file_name().into_string();
    if filename.is_err() {
      return Err(anyhow!("error: file name can not convert to string"));
    }
    let filename = filename.unwrap();

    info!("[START] {filename}");

    let depth_caps = depth_re
      .captures(&filename)
      .with_context(|| "error: file name")?;
    let z = depth_caps
      .name("z")
      .map(|m| m.as_str().parse::<usize>().unwrap())
      .with_context(|| "error")?;

    let obj = open_file(file.path())?;
    info!("[{filename}] open file");

    // 標準DICOM画像タグセット一覧 - 医療用デジタル画像と通信タグ
    // https://www.liberworks.co.jp/know/know_dicomTag.html
    // タグの意味
    // https://www.ihe-j.org/file2/n13/1.2_DICOM_Tanaka.pdf

    // 行
    rows = obj
      .element(Tag(0x0028, 0x0010))?
      .to_str()?
      .parse::<usize>()?;
    // 列
    columns = obj
      .element(Tag(0x0028, 0x0011))?
      .to_str()?
      .parse::<usize>()?;

    let pixel_data = obj.decode_pixel_data()?;
    let pixel_array = pixel_data.to_ndarray::<i16>()?;

    for (i, d) in pixel_array.iter().enumerate() {
      let x = i % rows;
      let y = i / rows;

      let data = Data {
        point: Point::new(x as u16, y as u16, z as u16),
        data: *d,
      };
      data_lst.push(data);
    }

    info!("[END] {filename}");
  }

  // 初期値の重心
  // 概ねの場所を指定しておくことでコントロールしたい
  let init_center_lst = vec![
    //胸腔
    Center {
      point: None,
      data: -990,
    },
    //肺組織
    Center {
      point: None,
      data: -750,
    },
    //脂肪
    Center {
      point: None,
      data: -53,
    },
    //血管
    Center {
      point: None,
      data: 34,
    },
    //骨
    Center {
      point: None,
      data: 300,
    },
  ];

  info!("[START] solve");
  // クラスタリング後の結果
  let solved = k_means::solve(
    calc_distance,
    calc_center,
    calc_eq,
    init_center_lst,
    &data_lst,
  )
  .await;
  info!("[END] solved");

  // 48枚目の画像を生成したい
  let depth = 48;
  let data_48 = solved
    .iter()
    .map(|l| {
      l.iter()
        .filter(|d| d.point.z == depth)
        .copied()
        .collect::<Vec<_>>()
    })
    .collect::<Vec<_>>();
  let img_48 = write_image::data_to_img(rows as u32, columns as u32, &data_48).await;
  info!("generate img");
  img_48.save("48.png")?;
  for (i, data) in data_48.iter().enumerate() {
    let p_l = data.iter().map(|d| d.point).collect::<Vec<Point>>();
    let p_l_1 = filter::opening(rows as i16, columns as i16, 48, &p_l, 1);
    let p_l_1 = filter::closing(rows as i16, columns as i16, 48, &p_l_1, 1);
    let img = write_image::point_to_img(rows as u32, columns as u32, &[p_l_1.clone()]).await;
    img.save(format!("48_{i}_oc.png"))?;
  }
  info!("all done");
  Ok(())
}
