use geometry::Aabb;

/// An acceleration structure over items with bounding boxes.
pub trait SpatialIndex<T> {
    fn new(items: impl IntoIterator<Item = (Aabb, T)>) -> Self;

    /// Calls `visit` for items in boxes whose `min_distance` is below a cutoff, in an
    /// implementation-defined order. The cutoff starts at infinity, and each call to `visit`
    /// replaces it with the returned value.
    ///
    /// `min_distance` returns at most the distance of any item inside a box, or `None` to skip
    /// the box. Any distance-like measure works, such as squared distance, if `visit` returns the
    /// same kind.
    fn traverse(&self, min_distance: impl Fn(&Aabb) -> Option<f64>, visit: impl FnMut(T) -> f64);
}
