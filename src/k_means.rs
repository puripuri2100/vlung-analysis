use tokio_stream::StreamExt;

pub async fn solve<T, C, F, G, E>(
  calc_distance: F,
  calc_center: G,
  calc_eq: E,
  init_center: Vec<C>,
  lst: &[T],
) -> Vec<Vec<T>>
where
  T: Sized + Clone,
  C: Sized + Clone,
  F: Fn(&C, &T) -> usize,
  G: Fn(&[T]) -> Option<C>,
  E: Fn(&[T], &[T]) -> bool,
{
  let n = init_center.len();
  let mut l1: Vec<Vec<T>> = Vec::new();
  let mut l2: Vec<Vec<T>> = vec![Vec::new(); n];
  let mut center_lst: Vec<C> = init_center;
  loop {
    let mut data_stream = tokio_stream::iter(lst);

    while let Some(data) = data_stream.next().await {
      // 一番近い重心のグループを選ぶ
      let (center_num, _) = center_lst
        .iter()
        .enumerate()
        .map(|(i, center)| (i, calc_distance(center, data)))
        .min_by_key(|(_, d)| *d)
        .unwrap();
      // 更新
      l2[center_num].push(data.clone());
    }

    if l1.iter().zip(l2.iter()).all(|(v1, v2)| calc_eq(v1, v2)) {
      // 変動しなくなったら終了
      break;
    } else {
      tracing::info!("loop");
      let mut new_center_list = Vec::new();
      for i in 0..l2.len() {
        let new_center_opt = calc_center(&l2[i]);
        if let Some(new_center) = new_center_opt {
          new_center_list.push(new_center)
        } else {
          new_center_list.push(center_lst[i].clone())
        }
      }
      //center_lst = l2.iter().map(|l| calc_center(l)).collect();
      center_lst = new_center_list;
      l1 = l2;
      l2 = vec![Vec::new(); n];
    }
  }
  l2
}
