use crate::Point;
use tokio_stream::StreamExt;

pub type GroupList = (Point, Vec<usize>);
pub type Block<T> = Vec<Vec<Vec<T>>>;

/// pointのリストから、どのグループに属しているのかのデータを生成するようにした
/// オープニングクロージングの過程でグループが複数個ありえるため、リストにしている
pub fn gen_blocks(
  rows: usize,
  columns: usize,
  height: usize,
  data: &[Vec<Point>],
) -> Block<GroupList> {
  let mut v = vec![vec![vec![(Point::new(0, 0, 0), vec![]); height]; columns]; rows];
  for (n, lst) in data.iter().enumerate() {
    for point in lst.iter() {
      v[point.x as usize][point.y as usize][point.z as usize] = (*point, vec![n]);
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
  } else if (point.x as usize) < rows {
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
  } else if (point.y as usize) < columns {
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
  } else if (point.z as usize) < height {
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
  let mut v = vec![vec![vec![(Point::new(0, 0, 0), vec![]); height]; columns]; rows];
  let mut yz_stream = tokio_stream::iter(data.clone());
  while let Some(yz) = yz_stream.next().await {
    let mut z_stream = tokio_stream::iter(yz);
    while let Some(z_data) = z_stream.next().await {
      let mut stream = tokio_stream::iter(z_data);
      while let Some((point, _)) = stream.next().await {
        // 和集合を取る
        let mut group = neighborhood(rows, columns, height, &point)
          .iter()
          .map(|p| {
            let lst = &data[p.x as usize][p.y as usize][p.z as usize].1;
            lst.clone()
          })
          .collect::<Vec<Vec<_>>>()
          .concat();
        group.sort();
        group.dedup();
        v[point.x as usize][point.y as usize][point.z as usize] = (point, group);
      }
    }
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
  let mut v = vec![vec![vec![(Point::new(0, 0, 0), vec![]); height]; columns]; rows];
  let mut yz_stream = tokio_stream::iter(data.clone());
  while let Some(yz) = yz_stream.next().await {
    let mut z_stream = tokio_stream::iter(yz);
    while let Some(z_data) = z_stream.next().await {
      let mut stream = tokio_stream::iter(z_data);
      while let Some((point, _)) = stream.next().await {
        // 積集合を取る
        let group_lst = neighborhood(rows, columns, height, &point)
          .iter()
          .map(|p| {
            let lst = &data[p.x as usize][p.y as usize][p.z as usize].1;
            lst.clone()
          })
          .collect::<Vec<Vec<_>>>();
        let mut group = Vec::new();
        for n in 0..group_size {
          if group_lst.iter().all(|g| g.iter().any(|n2| n == *n2)) {
            group.push(n);
          }
        }
        group.sort();
        v[point.x as usize][point.y as usize][point.z as usize] = (point, group);
      }
    }
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
  let mut v = data.to_vec();
  for _ in 0..n {
    v = erosion_block(rows, columns, height, &v, group_size).await;
  }
  for _ in 0..n {
    v = diation_block(rows, columns, height, &v).await;
  }
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
  let mut v = data.to_vec();
  for _ in 0..n {
    v = diation_block(rows, columns, height, &v).await;
  }
  for _ in 0..n {
    v = erosion_block(rows, columns, height, &v, group_size).await;
  }
  v
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
