use geometry::Aabb;

use crate::SpatialIndex;

const MAX_LEAF_ITEMS: usize = 4;

/// A bounding volume hierarchy, split at the median of each node's longest axis.
pub struct Bvh<T> {
    /// The root last. An interior node's children are adjacent.
    nodes: Vec<Node>,
    /// Ordered so that each leaf's items are contiguous.
    items: Vec<T>,
}

#[derive(Clone, Copy)]
struct Node {
    bounds: Aabb,
    /// A leaf's first item, or an interior node's first child.
    start: usize,
    /// A leaf's item count, or zero for an interior node.
    count: usize,
}

impl<T: Copy> SpatialIndex<T> for Bvh<T> {
    fn new(items: impl IntoIterator<Item = (Aabb, T)>) -> Self {
        let mut entries: Vec<_> = items.into_iter().collect();
        if entries.is_empty() {
            return Self {
                nodes: Vec::new(),
                items: Vec::new(),
            };
        }

        let mut nodes = Vec::new();
        let root = build(&mut nodes, &mut entries, 0);
        nodes.push(root);

        Self {
            nodes,
            items: entries.into_iter().map(|(_, item)| item).collect(),
        }
    }

    /// Depth-first, nearer child first.
    fn traverse(
        &self,
        min_distance: impl Fn(&Aabb) -> Option<f64>,
        mut visit: impl FnMut(T) -> f64,
    ) {
        let Some(root) = self.nodes.last() else {
            return;
        };

        let mut cutoff = f64::INFINITY;
        self.traverse_node(
            root,
            node_distance(&min_distance, root),
            &min_distance,
            &mut visit,
            &mut cutoff,
        );
    }
}

impl<T: Copy> Bvh<T> {
    fn traverse_node<F: FnMut(T) -> f64>(
        &self,
        node: &Node,
        distance: f64,
        min_distance: &impl Fn(&Aabb) -> Option<f64>,
        visit: &mut F,
        cutoff: &mut f64,
    ) {
        if distance >= *cutoff {
            return;
        }

        if node.count > 0 {
            for &item in &self.items[node.start..node.start + node.count] {
                *cutoff = visit(item);
                if distance >= *cutoff {
                    return;
                }
            }
            return;
        }

        let mut children = [&self.nodes[node.start], &self.nodes[node.start + 1]]
            .map(|child| (node_distance(min_distance, child), child));

        if children[1].0 < children[0].0 {
            children.swap(0, 1);
        }

        for (distance, child) in children {
            self.traverse_node(child, distance, min_distance, visit, cutoff);
        }
    }
}

/// Skipped boxes get an infinite distance, which no cutoff is above.
fn node_distance(min_distance: &impl Fn(&Aabb) -> Option<f64>, node: &Node) -> f64 {
    min_distance(&node.bounds).unwrap_or(f64::INFINITY)
}

/// Builds the subtree over `entries`, which start at item `offset`. Appends its descendants to
/// `nodes` and returns its root.
fn build<T>(nodes: &mut Vec<Node>, entries: &mut [(Aabb, T)], offset: usize) -> Node {
    let bounds = entries
        .iter()
        .map(|(bounds, _)| *bounds)
        .reduce(Aabb::union)
        .unwrap();

    if entries.len() <= MAX_LEAF_ITEMS {
        return Node {
            bounds,
            start: offset,
            count: entries.len(),
        };
    }

    let extent = bounds.max - bounds.min;
    let axis = (0..3)
        .max_by(|&a, &b| extent[a].total_cmp(&extent[b]))
        .unwrap();
    let center = |(bounds, _): &(Aabb, T)| bounds.center()[axis];

    // Partitions around the median center, without sorting either half.
    let middle = entries.len() / 2;
    entries.select_nth_unstable_by(middle, |a, b| center(a).total_cmp(&center(b)));

    let (left, right) = entries.split_at_mut(middle);
    let children = [
        build(nodes, left, offset),
        build(nodes, right, offset + middle),
    ];

    let start = nodes.len();
    nodes.extend(children);

    Node {
        bounds,
        start,
        count: 0,
    }
}

#[cfg(test)]
#[path = "../tests/unit/bvh.rs"]
mod tests;
