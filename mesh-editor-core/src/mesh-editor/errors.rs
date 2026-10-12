use std::fmt;

use super::MeshKey;

/// The mesh key is not registered with this editor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MeshNotFound {
    pub key: MeshKey,
}

impl fmt::Display for MeshNotFound {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "mesh {:?} is not registered", self.key)
    }
}

impl std::error::Error for MeshNotFound {}
