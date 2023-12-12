use crate::Point;

/// 膨張
/// 周辺8近傍の中に一つでも塗られていたら塗る
pub fn diation(rows: u16, columns: u16, data: &[Point]) -> Vec<Point> {
  let mut v = data
    .iter()
    .map(|point| {
      let x = point.x;
      let y = point.y;
      let z = point.z;
      let mut v = Vec::new();
      if 0 < x {
        v.push(Point::new(x - 1, y, z));
        if 0 < y {
          v.push(Point::new(x - 1, y - 1, z));
        }
        if y < columns {
          v.push(Point::new(x - 1, y + 1, z));
        }
      } else if x < rows {
        v.push(Point::new(x + 1, y, z));
        if 0 < y {
          v.push(Point::new(x + 1, y - 1, z));
        };
        if y < columns {
          v.push(Point::new(x + 1, y + 1, z));
        };
      } else {
        v.push(Point::new(x, y, z));
        if 0 < y {
          v.push(Point::new(x, y - 1, z));
        };
        if y < columns {
          v.push(Point::new(x, y + 1, z));
        };
      }
      v
    })
    .collect::<Vec<Vec<Point>>>()
    .concat();
  v.sort();
  v.dedup();
  v
}

/// 収縮
/// 周辺8近傍が全て塗られていないといけない
pub fn erosion(rows: u16, columns: u16, data: &[Point]) -> Vec<Point> {
  // 探索する範囲を既存の座標の周囲8近傍に限定する
  let mut lst = data
    .iter()
    .map(|point| {
      let x = point.x;
      let y = point.y;
      let z = point.z;
      let mut v = Vec::new();
      if 0 < x {
        v.push(Point::new(x - 1, y, z));
        if 0 < y {
          v.push(Point::new(x - 1, y - 1, z));
        }
        if y < columns {
          v.push(Point::new(x - 1, y + 1, z));
        }
      } else if x < rows {
        v.push(Point::new(x + 1, y, z));
        if 0 < y {
          v.push(Point::new(x + 1, y - 1, z));
        };
        if y < columns {
          v.push(Point::new(x + 1, y + 1, z));
        };
      } else {
        v.push(Point::new(x, y, z));
        if 0 < y {
          v.push(Point::new(x, y - 1, z));
        };
        if y < columns {
          v.push(Point::new(x, y + 1, z));
        };
      }
      v
    })
    .collect::<Vec<Vec<Point>>>()
    .concat();
  lst.sort();
  lst.dedup();
  let mut vec = Vec::new();
  for point in lst {
    let x = point.x;
    let y = point.y;
    let z = point.z;
    let mut point_lst = Vec::new();
    if 0 < x {
      point_lst.push(Point::new(x - 1, y, z));
      if 0 < y {
        point_lst.push(Point::new(x - 1, y - 1, z));
      }
      if y < columns {
        point_lst.push(Point::new(x - 1, y + 1, z));
      }
    } else if x < rows {
      point_lst.push(Point::new(x + 1, y, z));
      if 0 < y {
        point_lst.push(Point::new(x + 1, y - 1, z));
      };
      if y < columns {
        point_lst.push(Point::new(x + 1, y + 1, z));
      };
    } else {
      if 0 < y {
        point_lst.push(Point::new(x, y - 1, z));
      };
      if y < columns {
        point_lst.push(Point::new(x, y + 1, z));
      };
    }
    if point_lst
      .iter()
      .all(|p1| data.iter().any(|p2| p1.x == p2.x && p1.y == p2.y))
    {
      vec.push(point);
    }
  }
  println!("end!!");
  vec
}

/// 同じ回数分だけ収縮して膨張する
#[allow(dead_code)]
pub fn opening(rows: u16, columns: u16, data: &[Point], n: usize) -> Vec<Point> {
  let mut v = data.to_vec();
  for i in 0..n {
    println!("erosion {i}");
    v = erosion(rows, columns, &v);
    println!("end {i}");
  }
  for i in 0..n {
    println!("diation {i}");
    v = diation(rows, columns, &v);
    println!("end {i}");
  }
  v
}

/// 同じ回数分だけ膨張して収縮する
#[allow(dead_code)]
pub fn closing(rows: u16, columns: u16, data: &[Point], n: usize) -> Vec<Point> {
  let mut v = data.to_vec();
  for _ in 0..n {
    v = diation(rows, columns, &v);
  }
  for _ in 0..n {
    v = erosion(rows, columns, &v);
  }
  v
}
