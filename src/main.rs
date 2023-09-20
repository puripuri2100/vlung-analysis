use anyhow::{anyhow, Context, Result};
use clap::Parser;
use dicom::object::{open_file, Tag};
use dicom_pixeldata::PixelDecoder;
use regex::Regex;
use tokio::fs;
use tracing::*;

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
pub struct Data {
  pub x: u16,
  pub y: u16,
  pub z: u16,
  pub data: i16,
}

fn calc_distance(t1: &Data, t2: &Data) -> usize {
  (t1.data as usize).abs_diff(t2.data as usize)
}

fn calc_center(lst: &[Data]) -> Data {
  let len = lst.len();
  let d = (lst.iter().map(|d| d.data as i64).sum::<i64>() / len as i64) as i16;
  Data {
    x: 0,
    y: 0,
    z: 0,
    data: d,
  }
}

#[tokio::main]
async fn main() -> Result<()> {
  let args = Args::parse();

  init_logger().await?;

  let mut data_lst = Vec::new();

  let mut d1 = 0;
  let mut d2 = 0;
  let mut d3 = 0;
  let mut d4 = 0;
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

      if x == 102 && y == 252 && z == 48 {
        d1 = *d;
      } else if x == 48 && y == 142 && z == 48 {
        d2 = *d;
      } else if x == 327 && y == 268 && z == 48 {
        d3 = *d;
      } else if x == 379 && y == 350 && z == 48 {
        d4 = *d;
      }

      let data = Data {
        x: x as u16,
        y: y as u16,
        z: z as u16,
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
    Data {
      x: 102,
      y: 252,
      z: 48,
      data: d1,
    },
    //肺組織
    Data {
      x: 48,
      y: 142,
      z: 48,
      data: d2,
    },
    //血管
    Data {
      x: 327,
      y: 268,
      z: 48,
      data: d3,
    },
    //骨
    Data {
      x: 379,
      y: 350,
      z: 48,
      data: d4,
    },
  ];

  // クラスタリング後の結果
  let solved = k_means::solve(calc_distance, calc_center, init_center_lst, &data_lst).await;
  info!("solved");

  // 48枚目の画像を生成したい
  let depth = 48;
  let data_48 = solved
    .iter()
    .map(|l| {
      l.iter()
        .filter(|d| d.z == depth)
        .copied()
        .collect::<Vec<_>>()
    })
    .collect::<Vec<_>>();
  let img_48 = write_image::data_to_img(rows as u32, columns as u32, &data_48).await;
  info!("generate img");
  img_48.save("48.png")?;
  info!("all done");
  Ok(())
}
