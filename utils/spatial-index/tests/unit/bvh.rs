use glam::Vec3;

use super::*;

/// Small boxes scattered pseudo-randomly through a 100-unit cube.
fn scattered_boxes(count: usize) -> Vec<Aabb> {
    let mut seed = 1u32;
    let mut next = move || {
        seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        (seed >> 8) as f32 / (1 << 24) as f32 * 100.0
    };

    (0..count)
        .map(|_| {
            let min = Vec3::new(next(), next(), next());
            Aabb {
                min,
                max: min + next() * 0.05,
            }
        })
        .collect()
}

fn index(boxes: &[Aabb]) -> Bvh<usize> {
    Bvh::new(boxes.iter().copied().zip(0..))
}

fn visited(
    bvh: &Bvh<usize>,
    min_distance: impl Fn(&Aabb) -> Option<f64>,
    cutoff: f64,
) -> Vec<usize> {
    let mut visited = Vec::new();
    bvh.traverse(min_distance, |item| {
        visited.push(item);
        cutoff
    });
    visited
}

#[test]
fn traverse_visits_nothing_in_an_empty_index() {
    assert!(visited(&index(&[]), |_| Some(0.0), f64::INFINITY).is_empty());
}

#[test]
fn traverse_visits_every_item_once_without_a_cutoff() {
    let boxes = scattered_boxes(100);
    let mut visited = visited(&index(&boxes), |_| Some(0.0), f64::INFINITY);

    visited.sort_unstable();
    assert_eq!(visited, (0..100).collect::<Vec<_>>());
}

#[test]
fn traverse_skips_boxes_without_a_distance() {
    assert!(visited(&index(&scattered_boxes(100)), |_| None, f64::INFINITY).is_empty());
}

#[test]
fn traverse_stops_when_the_cutoff_is_below_every_distance() {
    let visited = visited(
        &index(&scattered_boxes(100)),
        |_| Some(0.0),
        f64::NEG_INFINITY,
    );

    assert_eq!(visited.len(), 1);
}

#[test]
fn traverse_finds_the_nearest_box_while_skipping_most() {
    let boxes = scattered_boxes(1000);
    let bvh = index(&boxes);
    let queries = scattered_boxes(50)
        .into_iter()
        .map(|aabb| aabb.min.as_dvec3() * 1.2 - 10.0);
    let mut visits = 0;

    for point in queries {
        let mut nearest = f64::INFINITY;
        bvh.traverse(
            |bounds| Some(bounds.distance_squared(point)),
            |item| {
                visits += 1;
                nearest = nearest.min(boxes[item].distance_squared(point));
                nearest
            },
        );

        let expected = boxes
            .iter()
            .map(|aabb| aabb.distance_squared(point))
            .fold(f64::INFINITY, f64::min);
        assert_eq!(nearest, expected);
    }

    assert!(visits < 50 * 1000 / 10, "visited {visits} items");
}
