use crate::Point;

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
