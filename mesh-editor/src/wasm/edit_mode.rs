//! JS representation of [`EditMode`]: JS sets and reads modes as component masks.

use mesh::{ComponentMask, ComponentType};
use wasm_bindgen::prelude::*;

use crate::EditMode;

// Declared here, not in `mesh`: a dependency crate's TypeScript can be dropped from dev builds.
#[wasm_bindgen(typescript_custom_section)]
const COMPONENT_MASK: &'static str = r#"
/**
 * A set of component types, where each type's bit is `1 << ComponentType`. Test for a type
 * with `mask & (1 << ComponentType.Edge)`; `0` is the empty set.
 */
export type ComponentMask = number;
"#;

// JS builds masks from `ComponentType` values, so each flag must be `1 << ComponentType`.
const _: () = {
    assert!(ComponentMask::VERTEX.bits() == 1 << ComponentType::Vertex as u8);
    assert!(ComponentMask::EDGE.bits() == 1 << ComponentType::Edge as u8);
    assert!(ComponentMask::FACE.bits() == 1 << ComponentType::Face as u8);
};

impl EditMode {
    /// Converts a JS `ComponentMask`, rejecting unknown bits.
    pub(super) fn from_js_components(components: u32) -> Result<Self, JsError> {
        u8::try_from(components)
            .ok()
            .and_then(ComponentMask::from_bits)
            .map(Self::from_components)
            .ok_or_else(|| JsError::new(&format!("invalid component mask {components}")))
    }
}
