//! The boundary of a selection.

use crate::{FaceKey, VertKey};

/// A selection's boundary: the selected vertices touching unselected geometry,
/// plus the vertex and face rings just inside and outside them.
pub struct SelectionBoundary {}

impl SelectionBoundary {
    pub fn vertices(&self) -> impl Iterator<Item = VertKey> + '_ {
        std::iter::empty::<VertKey>()
    }

    pub fn inner_vertices(&self) -> impl Iterator<Item = VertKey> + '_ {
        std::iter::empty::<VertKey>()
    }

    pub fn outer_vertices(&self) -> impl Iterator<Item = VertKey> + '_ {
        std::iter::empty::<VertKey>()
    }

    pub fn inner_faces(&self) -> impl Iterator<Item = FaceKey> + '_ {
        std::iter::empty::<FaceKey>()
    }

    pub fn outer_faces(&self) -> impl Iterator<Item = FaceKey> + '_ {
        std::iter::empty::<FaceKey>()
    }
}
