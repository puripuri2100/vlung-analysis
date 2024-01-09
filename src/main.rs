use anyhow::{anyhow, Context, Result};
use clap::Parser;
use dicom::object::{open_file, Tag};
use dicom_pixeldata::PixelDecoder;
use regex::Regex;
use serde::{Deserialize, Serialize};
use tokio::fs::{self, File};
use tokio::io::AsyncWriteExt;
use tokio_stream::StreamExt;
use tracing::*;

mod filter;
mod k_means;
mod write_image;

#[derive(Parser)]
#[command(author, version)]
struct Args {
  /// CTファイルのあるフォルダへのパス
  #[arg(short, long)]
  folder: String,
  /// 生成するJSONファイルのパス
  #[arg(short, long)]
  output: String,
}

async fn init_logger() -> Result<()> {
  let subscriber = tracing_subscriber::fmt()
    .with_max_level(tracing::Level::INFO)
    .finish();
  tracing::subscriber::set_global_default(subscriber)?;
  Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Deserialize, Serialize)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Deserialize, Serialize)]
pub struct Data {
  pub point: Point,
  pub data: i16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Deserialize, Serialize)]
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

async fn write_data(file: &mut File, rows: usize, columns: usize, height: usize, group_size: usize, block: &filter::Block<filter::GroupList>) -> Result<()> {
  if group_size > 10 {
    return Err(anyhow!("unsupported size"))
  }
  file.write_all(format!("{rows} {columns} {height} {group_size}\n").as_bytes()).await?;
  let mut xy_stream = tokio_stream::iter(block);
  while let Some(xy) = xy_stream.next().await {
    let mut is_first = true;
    let mut s = String::new();
    let mut z = 0;
    let mut now_group;
    let mut group_len;
    for x_lst in xy.iter() {
      now_group = 0;
      group_len = 0;
      for data in x_lst.iter() {
        if let Some((point, group)) = data {
          if is_first {
            z = point.z;
            is_first = false;
          }
          if let Some(g) = group.iter().min() {
            if now_group == *g {
              group_len += 1;
            } else {
              // 書き出し処理
              s.push_str(&format!("{now_group}*{group_len},"));
              // 初期化
              now_group = *g;
              group_len = 1;
            }
          }
        } else {
          // 数字が無い場合は0として扱う
          if now_group == 0 {
            group_len += 1;
          } else {
            now_group = 0;
            group_len = 1;
          }
        }
      }
      s.push_str(&format!("{now_group}*{group_len}\n"));
    }
    file.write_all(format!("{z}\n{s}").as_bytes()).await?;
  }
  Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
  let args = Args::parse();

  init_logger().await?;

  let mut data_lst = Vec::new();
  let depth_re = Regex::new(r"[^\d]*(?<z>\d+)[^\d]*").unwrap();
  let mut rows = 0;
  let mut columns = 0;
  let mut z_lst = Vec::new();
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
    z_lst.push(z);

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

  let height: usize = *z_lst.iter().max().unwrap_or(&0) + 1;
  let group_size = solved.len();

  let point_lst = solved
    .iter()
    .map(|l| l.iter().map(|d| d.point).collect())
    .collect::<Vec<Vec<Point>>>();
  let block_data_raw = filter::gen_blocks(rows, columns, height, &point_lst);
  // ノイズ除去をする
  let block_data =
    filter::opening_block(rows, columns, height, &block_data_raw, group_size, 1).await;
  // 穴埋めをする
  let block_data = filter::closing_block(rows, columns, height, &block_data, group_size, 1).await;

  // 48枚目の画像を生成したい
  let depth = 48;

  // 元データ
  info!("[START] generate raw img");
  let mut data_raw_48 = vec![vec![]; group_size];
  for xy in block_data_raw[depth].iter() {
    for x in xy.iter() {
      if let Some((point, group)) = &x {
        let mut group = group.clone();
        group.sort();
        if let Some(g) = group.first() {
          data_raw_48[*g].push(*point);
        }
      }
    }
  }
  let img_48 = write_image::point_to_img(rows as u32, columns as u32, &data_raw_48).await;
  img_48.save("48_raw.png")?;
  for (i, data) in data_raw_48.iter().enumerate() {
    let img = write_image::point_to_img(rows as u32, columns as u32, &[data.clone()]).await;
    img.save(format!("48_raw_{i}.png"))?;
  }
  info!("[END] generate raw img");

  // オープニング・クロージングした後
  info!("[START] generate oc img");
  let mut data_48 = vec![vec![]; group_size];
  for xy in block_data_raw[depth].iter() {
    for x in xy.iter() {
      if let Some((point, group)) = &x {
        let mut group = group.clone();
        group.sort();
        if let Some(g) = group.first() {
          data_48[*g].push(*point);
        }
      }
    }
  }
  let img_48 = write_image::point_to_img(rows as u32, columns as u32, &data_48).await;
  img_48.save("48.png")?;
  for (i, data) in data_48.iter().enumerate() {
    let img = write_image::point_to_img(rows as u32, columns as u32, &[data.clone()]).await;
    img.save(format!("48_{i}.png"))?;
  }
  info!("[End] generate oc img");

  info!("[START] generate data file");
  let mut buf = File::create(&args.output).await?;
  write_data(&mut buf, rows, columns, height, group_size, &block_data).await?;
  info!("[END] generate data file");

  info!("all done");
  Ok(())
}
