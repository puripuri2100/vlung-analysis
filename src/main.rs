//! CT画像データを解析し、各部位ごとのOBJファイルを生成するソフトウェア
//!
//! <div style="text-align: center">
//!   <img src="img.png">
//! </div>
//!
//! # インストール方法
//!
//! ## 必要なソフトウェア
//!
//!  - [Rust](https://www.rust-lang.org/)
//!
//! ## インストール方法
//!
//! Rustの管理ツールであるCargoを[公式サイトのインストールページ](https://www.rust-lang.org/tools/install)に従ってインストールしてください。
//!
//! 次にcargoを用いてインストールします。
//!
//! ```sh
//! cargo install vlung-analysis
//! ```
//!
//! 最新版を使いたい場合はリポジトリを手元にcloneしてからインストをします。
//!
//! ```sh
//! git clone https://github.com/puripuri2100/vlung-analysis.git
//! cargo install --path vlung-analysis
//! ```
//!
//! 現在Windowsではライブラリの都合でうまくインストールできないことを確認しています。そのため、リポジトリをcloneしたうえで
//!
//! ```
//! cargo run --release -- --help
//! ```
//!
//! のようにして起動するようにしてください。
//!
//! # 使い方
//!
//! 引数がいくつかあります。詳しくは
//!
//! ```sh
//! vlung-analysis --help
//! ```
//!
//! を行ってください。
//!
//! ## 必須引数
//!
//! - `-f`, `--folder`：CTファイルのあるフォルダへのパス
//! - `-o`, `--output`：生成するファイルへのパス
//!
//! ## オプション引数
//!
//! - `-d`, `--depth-img`：肺の断面画像をその場に生成します。そのときの断面の深さを与えます
//! - `-s`, `--start-range`：解析範囲を直方体の大きさに制限することができます。そのときの始点の座標です。
//! - `-e`, `--end-range`：解析範囲を直方体の大きさに制限することができます。そのときの終点の座標です。
//! - `-n`, `--noise-removal`：ノイズ除去をするときの回数です。数が大きくなればなるほどノイズが除去されますが、必要な部分も消える可能性があります。
//! - `-i`, `--init-colors`：部位を分割する際の基準値を与えることができます。
//!
//! # CT画像データの取得方法
//!
//! 自分自身のCT画像データを持っている方は少ないと思うので、試しに使ってみたい方はCC0ライセンスのもとで自分が公開している気胸時の肺のCT画像データを使ってみてください：<https://github.com/puripuri2100/lung>
//!
//! 自分自身のCT画像データを取得してみたい方は撮影された病院の制度を利用して入手してみてください。例えば筑波大学附属病院では「診療記録等の開示」という制度が用意されています。：<https://www.hosp.tsukuba.ac.jp/outpatient/outpatient/release.html>
//!
//! ---
//!
//! [The MIT License](https://github.com/puripuri2100/vlung-analysis/blob/master/LICENSE)
//!
//! Copyright (c) 2024 Naoki Kaneko (a.k.a. "puripuri2100")
//!

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
mod marching_cubes;
mod write_image;

#[derive(Parser)]
#[command(author, version)]
struct Args {
  /// CTファイルのあるフォルダへのパス
  #[arg(short, long)]
  folder: String,
  /// 生成するファイルのパス
  #[arg(short, long)]
  output: String,
  /// 生成する画像の深さ
  #[arg(short, long)]
  depth_img: Option<usize>,
  /// 解析を行う範囲の座標のスタート
  #[arg(short, long, value_delimiter = ' ', num_args = 3)]
  start_range: Option<Vec<usize>>,
  /// 解析を行う範囲の座標の終点
  #[arg(short, long, value_delimiter = ' ', num_args = 3)]
  end_range: Option<Vec<usize>>,
  /// ノイズ除去の濃さを数値で与える
  #[arg(short, long, default_value = "1")]
  noise_removal: usize,
  /// 部位を分割する際に与える初期値です
  /// 先頭の値はデフォルト値として内部で扱われます
  #[arg(short, long, value_delimiter = ' ', num_args = 2..)]
  init_colors: Option<Vec<i16>>,
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

      let mut color_data = *d;
      if let Some(start_range) = &args.start_range {
        if start_range[0] > x || start_range[1] > y || start_range[2] > z {
          // 範囲外なので真っ黒にする
          color_data = -1000
        }
      }
      if let Some(end_range) = &args.end_range {
        if end_range[0] < x || end_range[1] < y || end_range[2] < z {
          // 範囲外なので真っ黒にする
          color_data = -1000
        }
      }
      let data = Data {
        point: Point::new(x as u16, y as u16, z as u16),
        data: color_data,
      };
      data_lst.push(data);
    }

    info!("[END] {filename}");
  }

  // 初期値の重心
  // 概ねの場所を指定しておくことでコントロールしたい
  let init_center_lst = if let Some(v) = args.init_colors {
    v.iter()
      .map(|i| Center {
        point: None,
        data: *i,
      })
      .collect()
  } else {
    vec![
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
    ]
  };

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
  let block_data = filter::opening_block(
    rows,
    columns,
    height,
    &block_data_raw,
    group_size,
    args.noise_removal,
  )
  .await;
  // 穴埋めをする
  let block_data = filter::closing_block(
    rows,
    columns,
    height,
    &block_data,
    group_size,
    args.noise_removal,
  )
  .await;

  if let Some(depth) = args.depth_img {
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
    img_48.save(format!("{depth}_raw.png"))?;
    for (i, data) in data_raw_48.iter().enumerate() {
      let img = write_image::point_to_img(rows as u32, columns as u32, &[data.clone()]).await;
      img.save(format!("{depth}_raw_{i}.png"))?;
    }
    info!("[END] generate raw img");

    // オープニング・クロージングした後
    info!("[START] generate oc img");
    let mut data_48 = vec![vec![]; group_size];
    for xy in block_data[depth].iter() {
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
    img_48.save(format!("{depth}.png"))?;
    for (i, data) in data_48.iter().enumerate() {
      let img = write_image::point_to_img(rows as u32, columns as u32, &[data.clone()]).await;
      img.save(format!("{depth}_{i}.png"))?;
    }
    info!("[End] generate oc img");
  }

  info!("[START] marching_cubes");
  let obj_data_lst =
    marching_cubes::marching_cubes(rows, columns, height, group_size, &block_data).await;
  let obj_data_iter = obj_data_lst.iter().enumerate();
  info!("[END] marching_cubes");
  let mut obj_data_stream = tokio_stream::iter(obj_data_iter);
  while let Some((i, obj_data)) = obj_data_stream.next().await {
    if i != 0 {
      info!("[START] write obj file({i})");
      let mut buf = File::create(format!("{}_{i}.obj", &args.output)).await?;
      let (v_lst, f_lst) = obj_data;
      for (x, y, z) in v_lst.iter() {
        buf.write_all(format!("v {x} {y} {z}\n").as_bytes()).await?;
      }
      let mut f_stream = tokio_stream::iter(f_lst);
      while let Some((v1, v2, v3)) = f_stream.next().await {
        buf
          .write_all(format!("f {v1} {v2} {v3}\n").as_bytes())
          .await?;
      }
      info!("[END] write obj file({i})");
    }
  }

  /*
  info!("[START] generate data file");
  let mut buf = File::create(&args.output).await?;
  write_data(&mut buf, rows, columns, height, group_size, &block_data).await?;
  info!("[END] generate data file");
  */

  info!("all done");
  Ok(())
}
