use crate::Point;
use tokio_stream::StreamExt;
use tracing::*;

pub type GroupList = (Point, Vec<usize>);
pub type Block<T> = Vec<Vec<Vec<Option<T>>>>;

/// pointのリストから、どのグループに属しているのかのデータを生成するようにした
/// オープニングクロージングの過程でグループが複数個ありえるため、リストにしている
pub fn gen_blocks(
  rows: usize,
  columns: usize,
  height: usize,
  data: &[Vec<Point>],
) -> Block<GroupList> {
  let mut v = vec![vec![vec![None; height]; columns]; rows];
  for (n, lst) in data.iter().enumerate() {
    for point in lst.iter() {
      v[point.x as usize][point.y as usize][point.z as usize] = Some((*point, vec![n]));
    }
  }
  v
}

pub async fn blocks_to_points(blocks: Block<GroupList>, group_size: usize) -> Vec<Vec<Point>> {
  let mut v = vec![Vec::new(); group_size];
  let mut yz_stream = tokio_stream::iter(blocks);
  while let Some(yz) = yz_stream.next().await {
    let mut z_stream = tokio_stream::iter(yz);
    while let Some(z) = z_stream.next().await {
      let mut group_stream = tokio_stream::iter(z);
      while let Some(grouplit_opt) = group_stream.next().await {
        if let Some((point, group)) = grouplit_opt {
          let mut group = group.clone();
          group.sort();
          if let Some(g) = group.first() {
            v[*g].push(point);
          }
        }
      }
    }
  }
  v
}

/// 境界チェックをした上で近傍のリストを生成
/// 一旦周囲6近傍で
pub fn neighborhood(rows: usize, columns: usize, height: usize, point: &Point) -> Vec<Point> {
  let mut v = Vec::new();
  if 0 < point.x as usize {
    v.push(Point {
      x: point.x - 1,
      ..*point
    });
  }
  if (point.x as usize) < rows - 1 {
    v.push(Point {
      x: point.x + 1,
      ..*point
    });
  }

  if 0 < point.y as usize {
    v.push(Point {
      y: point.y - 1,
      ..*point
    });
  }
  if (point.y as usize) < columns - 1 {
    v.push(Point {
      y: point.y + 1,
      ..*point
    });
  }

  if 0 < point.z as usize {
    v.push(Point {
      z: point.z - 1,
      ..*point
    });
  }
  if (point.z as usize) < height - 1 {
    v.push(Point {
      z: point.z + 1,
      ..*point
    });
  }

  v
}

/// 3次元での膨張処理
/// 周囲6近傍のグループの和集合
/// 周囲26近傍まで伸ばすかは要検討
pub async fn diation_block(
  rows: usize,
  columns: usize,
  height: usize,
  data: &Block<GroupList>,
) -> Block<GroupList> {
  let mut v = vec![vec![vec![None; height]; columns]; rows];
  let mut yz_stream = tokio_stream::iter(data.clone());
  while let Some(yz) = yz_stream.next().await {
    info!("[START] diation x_i");
    let mut z_stream = tokio_stream::iter(yz);
    while let Some(z_data) = z_stream.next().await {
      let mut stream = tokio_stream::iter(z_data);
      while let Some(grouplit_opt) = stream.next().await {
        if let Some((point, _)) = grouplit_opt {
          // 和集合を取る
          let mut group = neighborhood(rows, columns, height, &point)
            .iter()
            .map(|p| {
              if let Some((_, lst)) = &data[p.x as usize][p.y as usize][p.z as usize] {
                lst.clone()
              } else {
                Vec::new()
              }
            })
            .collect::<Vec<Vec<_>>>()
            .concat();
          group.sort();
          group.dedup();
          v[point.x as usize][point.y as usize][point.z as usize] = Some((point, group));
        }
      }
    }
    info!("[END] diation x_i");
  }
  v
}

/// 3次元での収縮処理
/// 周囲6近傍のグループの積集合
/// 周囲26近傍まで伸ばすかは要検討
pub async fn erosion_block(
  rows: usize,
  columns: usize,
  height: usize,
  data: &Block<GroupList>,
  group_size: usize,
) -> Block<GroupList> {
  let mut v = vec![vec![vec![None; height]; columns]; rows];
  let mut yz_stream = tokio_stream::iter(data.clone());
  while let Some(yz) = yz_stream.next().await {
    info!("[START] erosion x_i");
    let mut z_stream = tokio_stream::iter(yz);
    while let Some(z_data) = z_stream.next().await {
      let mut stream = tokio_stream::iter(z_data);
      while let Some(grouplit_opt) = stream.next().await {
        if let Some((point, _)) = grouplit_opt {
          // 積集合を取る
          let group_lst = neighborhood(rows, columns, height, &point)
            .iter()
            .map(|p| {
              if let Some((_, lst)) = &data[p.x as usize][p.y as usize][p.z as usize] {
                lst.clone()
              } else {
                Vec::new()
              }
            })
            .collect::<Vec<Vec<_>>>();
          let mut group = Vec::new();
          for n in 0..group_size {
            if group_lst
              .iter()
              .filter(|g| !g.is_empty())
              .all(|g| g.iter().any(|n2| n == *n2))
            {
              group.push(n);
            }
          }
          group.sort();
          v[point.x as usize][point.y as usize][point.z as usize] = Some((point, group));
        }
      }
    }
    info!("[END] erosion x_i");
  }
  v
}

/// 同じ回数分だけ収縮して膨張する
#[allow(dead_code)]
pub async fn opening_block(
  rows: usize,
  columns: usize,
  height: usize,
  data: &Block<GroupList>,
  group_size: usize,
  n: usize,
) -> Block<GroupList> {
  info!("[START] opening block");
  let mut v = data.to_vec();
  for _ in 0..n {
    v = erosion_block(rows, columns, height, &v, group_size).await;
  }
  for _ in 0..n {
    v = diation_block(rows, columns, height, &v).await;
  }
  info!("[END] opening block");
  v
}

/// 同じ回数分だけ膨張して収縮する
#[allow(dead_code)]
pub async fn closing_block(
  rows: usize,
  columns: usize,
  height: usize,
  data: &Block<GroupList>,
  group_size: usize,
  n: usize,
) -> Block<GroupList> {
  info!("[START] closing block");
  let mut v = data.to_vec();
  for _ in 0..n {
    v = diation_block(rows, columns, height, &v).await;
  }
  for _ in 0..n {
    v = erosion_block(rows, columns, height, &v, group_size).await;
  }
  info!("[END] closing block");
  v
}

#[cfg(test)]
mod block_test {
  use crate::filter::*;
  use crate::Point;
  #[test]
  fn check_gen_blocks() {
    let data = vec![
      vec![Point::new(2, 2, 2), Point::new(2, 2, 3)],
      vec![Point::new(2, 3, 2)],
    ];
    let rows = 4;
    let columns = 4;
    let height = 5;
    let gen_blocks = gen_blocks(rows, columns, height, &data);
    let blocks = vec![
      // x == 0
      vec![vec![None; height]; columns],
      // x == 1
      vec![vec![None; height]; columns],
      // x == 2
      vec![
        // y == 0
        vec![None; height],
        // y == 1
        vec![None; height],
        // y == 2
        vec![
          None,
          None,
          Some((Point::new(2, 2, 2), vec![0])),
          Some((Point::new(2, 2, 3), vec![0])),
          None,
        ],
        // y == 3
        vec![None, None, Some((Point::new(2, 3, 2), vec![1])), None, None],
      ],
      // x == 3
      vec![vec![None; height]; columns],
    ];
    assert_eq!(gen_blocks, blocks);
  }

  #[tokio::test]
  async fn check_blocks_to_points_1() {
    let data = vec![
      vec![Point::new(2, 2, 2), Point::new(2, 2, 3)],
      vec![Point::new(2, 3, 2)],
    ];
    let rows = 4;
    let columns = 4;
    let height = 5;
    let group_size = 2;
    let gen = blocks_to_points(gen_blocks(rows, columns, height, &data), group_size).await;
    assert_eq!(gen, data);
  }

  #[tokio::test]
  async fn check_blocks_to_points_2() {
    let data = vec![vec![Point::new(0, 0, 0)]];
    let group_size = 1;
    let gen = blocks_to_points(
      vec![vec![vec![Some((Point::new(0, 0, 0), vec![0]))]]],
      group_size,
    )
    .await;
    assert_eq!(gen, data);
  }

  #[tokio::test]
  async fn check_blocks_to_points_3() {
    let data = vec![vec![], vec![Point::new(0, 0, 0)]];
    let group_size = 2;
    let gen = blocks_to_points(
      vec![vec![vec![Some((Point::new(0, 0, 0), vec![1]))]]],
      group_size,
    )
    .await;
    assert_eq!(gen, data);
  }

  #[test]
  fn check_neighborhood_1() {
    let rows = 4;
    let columns = 4;
    let height = 5;
    let mut gen = neighborhood(rows, columns, height, &Point::new(0, 0, 0));
    let mut expectation = vec![
      Point::new(0, 0, 1),
      Point::new(0, 1, 0),
      Point::new(1, 0, 0),
    ];
    gen.sort();
    expectation.sort();
    assert_eq!(gen, expectation);
  }

  #[test]
  fn check_neighborhood_2() {
    let rows = 4;
    let columns = 4;
    let height = 5;
    let mut gen = neighborhood(rows, columns, height, &Point::new(1, 1, 1));
    let mut expectation = vec![
      Point::new(1, 1, 2),
      Point::new(1, 1, 0),
      Point::new(1, 2, 1),
      Point::new(1, 0, 1),
      Point::new(2, 1, 1),
      Point::new(0, 1, 1),
    ];
    gen.sort();
    expectation.sort();
    assert_eq!(gen, expectation);
  }

  #[test]
  fn check_neighborhood_3() {
    let rows = 4;
    let columns = 4;
    let height = 5;
    let mut gen = neighborhood(rows, columns, height, &Point::new(3, 3, 1));
    let mut expectation = vec![
      Point::new(3, 3, 0),
      Point::new(3, 3, 2),
      Point::new(3, 2, 1),
      Point::new(2, 3, 1),
    ];
    gen.sort();
    expectation.sort();
    assert_eq!(gen, expectation);
  }
}

/// 膨張
/// 周辺8近傍の中に一つでも塗られていたら塗る
pub fn diation(rows: i16, columns: i16, z: u16, data: &[Point]) -> Vec<Point> {
  let mut v = Vec::new();
  for x in 0..rows {
    for y in 0..columns {
      let point_lst = [
        (x - 1, y, z),
        (x + 1, y, z),
        (x, y - 1, z),
        (x, y + 1, z),
        (x - 1, y - 1, z),
        (x + 1, y + 1, z),
        (x - 1, y + 1, z),
        (x + 1, y - 1, z),
      ]
      .iter()
      .filter(|(x, y, _)| *x >= 0 && *y >= 0)
      .map(|(x, y, z)| Point::new(*x as u16, *y as u16, *z))
      .collect::<Vec<Point>>();
      let point = Point::new(x as u16, y as u16, z);
      if point_lst
        .iter()
        .any(|p1| data.iter().any(|p2| p1.x == p2.x && p1.y == p2.y))
      {
        v.push(point);
      }
    }
  }
  v
}

/// 収縮
/// 周辺8近傍が全て塗られていないといけない
pub fn erosion(rows: i16, columns: i16, z: u16, data: &[Point]) -> Vec<Point> {
  let mut v = Vec::new();
  for x in 0..rows {
    for y in 0..columns {
      let point_lst = [
        (x - 1, y, z),
        (x + 1, y, z),
        (x, y - 1, z),
        (x, y + 1, z),
        (x - 1, y - 1, z),
        (x + 1, y + 1, z),
        (x - 1, y + 1, z),
        (x + 1, y - 1, z),
      ]
      .iter()
      .filter(|(x, y, _)| *x >= 0 && *y >= 0)
      .map(|(x, y, z)| Point::new(*x as u16, *y as u16, *z))
      .collect::<Vec<Point>>();
      let point = Point::new(x as u16, y as u16, z);
      if point_lst
        .iter()
        .all(|p1| data.iter().any(|p2| p1.x == p2.x && p1.y == p2.y))
      {
        v.push(point);
      }
    }
  }
  v
}

/// 同じ回数分だけ収縮して膨張する
#[allow(dead_code)]
pub fn opening(rows: i16, columns: i16, z: u16, data: &[Point], n: usize) -> Vec<Point> {
  let mut v = data.to_vec();
  for _ in 0..n {
    v = erosion(rows, columns, z, &v);
  }
  for _ in 0..n {
    v = diation(rows, columns, z, &v);
  }
  v
}

/// 同じ回数分だけ膨張して収縮する
#[allow(dead_code)]
pub fn closing(rows: i16, columns: i16, z: u16, data: &[Point], n: usize) -> Vec<Point> {
  let mut v = data.to_vec();
  for _ in 0..n {
    v = diation(rows, columns, z, &v);
  }
  for _ in 0..n {
    v = erosion(rows, columns, z, &v);
  }
  v
}
