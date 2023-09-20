use tokio_stream::StreamExt;

pub async fn solve<T, F, G>(
  calc_distance: F,
  calc_center: G,
  init_center: Vec<T>,
  lst: &[T],
) -> Vec<Vec<T>>
where
  T: Sized + Clone + Eq + Ord,
  F: Fn(&T, &T) -> usize,
  G: Fn(&[T]) -> T,
{
  let n = init_center.len();
  let mut l1: Vec<Vec<T>> = Vec::new();
  let mut l2: Vec<Vec<T>> = vec![Vec::new(); n];
  let mut center_lst: Vec<T> = init_center;
  loop {
    let mut data_stream = tokio_stream::iter(lst);

    while let Some(data) = data_stream.next().await {
      // 一番近い重心のグループを選ぶ
      let (center_num, _) = center_lst
        .iter()
        .enumerate()
        .map(|(i, t)| (i, calc_distance(t, data)))
        .min_by_key(|(_, d)| *d)
        .unwrap();
      // 更新
      l2[center_num].push(data.clone());
    }

    for l in l2.iter_mut() {
      l.sort();
    }

    if l1 == l2 {
      // 変動しなくなったら終了
      break;
    } else {
      center_lst = l2.iter().map(|l| calc_center(l)).collect();
      l1 = l2;
      l2 = vec![Vec::new(); n];
    }
  }
  l2
}
