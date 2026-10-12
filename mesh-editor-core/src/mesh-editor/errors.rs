use std::fmt;

use crate::{EditMode, ToolId};

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

/// A requested tool could not be activated.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolActivationError {
    NotFound { id: ToolId },
    IncompatibleMode { id: ToolId, mode: EditMode },
}

impl fmt::Display for ToolActivationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound { id } => write!(formatter, "tool {id:?} is not registered"),
            Self::IncompatibleMode { id, mode } => {
                write!(formatter, "tool {id:?} does not support edit mode {mode:?}")
            }
        }
    }
}

impl std::error::Error for ToolActivationError {}
