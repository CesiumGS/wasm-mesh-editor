//! JS adapters for pointer input: the `PointerInput` JS object and its conversion to
//! [`PointerInput`].

use glam::{DMat4, DVec3};
use mesh::ComponentKey;
use serde::Deserialize;
use slotmap::KeyData;
use tsify::{Ts, Tsify};
use wasm_bindgen::prelude::*;

use super::super::mesh_editor::ComponentKeysJs;
use crate::{
    MeshEditor, MeshKey, MeshTransform, Modifiers, PickHit, PickResult, PointerInput, SceneInput,
    ToolAccessor, WorldRay,
};

// Doc comments on these JS-facing types also document the generated TypeScript.

/// Any object with numeric `x`, `y`, `z`, such as a Cesium `Cartesian3`.
#[derive(Deserialize, Tsify)]
pub struct Vector3Like {
    x: f64,
    y: f64,
    z: f64,
}

/// Keyboard modifiers held during the event. Omitted flags are `false`.
#[derive(Deserialize, Tsify, Default)]
#[serde(default)]
pub struct PointerModifiers {
    shift: bool,
    control: bool,
    alt: bool,
    meta: bool,
}

/// The components picked on one mesh, with keys from its `OverlayViews.componentKeys`.
#[derive(Deserialize, Tsify)]
#[serde(rename_all = "camelCase")]
#[tsify(rename = "PickHit")]
pub struct PickHitJs {
    #[tsify(type = "bigint")]
    mesh: u64,
    #[tsify(type = "ComponentKeys")]
    component_keys: ComponentKeysJs,
}

/// World-space ray, such as a Cesium `Ray`. The direction is normalized.
#[derive(Deserialize, Tsify)]
pub struct RayLike {
    origin: Vector3Like,
    direction: Vector3Like,
}

// Converted to the Rust `SceneInput` for tools.
#[derive(Deserialize, Tsify)]
#[serde(rename_all = "camelCase")]
#[tsify(rename = "SceneInput")]
pub struct SceneInputJs {
    /// World-space camera direction, pointing into the scene. Normalized.
    view_direction: Vector3Like,
    /// Meshes whose placements are given in `matrices`.
    #[serde(with = "serde_wasm_bindgen::preserve")]
    #[tsify(type = "BigUint64Array")]
    mesh_keys: js_sys::BigUint64Array,
    /// One column-major 4x4 local-to-world matrix (16 numbers) per entry in `meshKeys`.
    #[serde(with = "serde_wasm_bindgen::preserve")]
    #[tsify(type = "Float64Array")]
    matrices: js_sys::Float64Array,
}

/// Renderer-resolved pointer data. All fields are optional.
// Converted to the Rust `PointerInput` for tools.
#[derive(Deserialize, Tsify)]
#[tsify(rename = "PointerInput")]
pub struct PointerInputJs {
    #[serde(default)]
    modifiers: PointerModifiers,
    /// The picked components, one entry per mesh. Omit `pick` if picking wasn't performed, and
    /// pass `[]` if picking found nothing.
    #[tsify(optional, type = "PickHit[]")]
    pick: Option<Vec<PickHitJs>>,
    #[tsify(optional)]
    ray: Option<RayLike>,
    #[tsify(optional, type = "SceneInput")]
    scene: Option<SceneInputJs>,
}

pub(in crate::wasm) fn dispatch_pointer(
    editor: &mut MeshEditor,
    input: Ts<PointerInputJs>,
    handler: impl FnOnce(&mut ToolAccessor<'_>, &PointerInput<'_>) -> bool,
) -> Result<bool, JsError> {
    // `Tsify::from_js` rather than `Ts::to_rust`, whose errors name internal Rust types.
    let input = PointerInputJs::from_js(input.js_value())?;

    let picked = input.pick.map(picked_components);
    let mesh_hits: Vec<PickHit<'_>> = picked
        .iter()
        .flatten()
        .map(|(mesh, components)| PickHit {
            mesh: *mesh,
            components,
        })
        .collect();
    let pick = match picked {
        None => PickResult::NotPerformed,
        Some(_) => PickResult::Hits(&mesh_hits),
    };
    let ray = input.ray.map(world_ray).transpose()?;
    let transforms = match &input.scene {
        Some(scene) => mesh_transforms(scene)?,
        None => Vec::new(),
    };
    let scene = match &input.scene {
        Some(scene) => Some(SceneInput {
            view_direction: unit_vector(&scene.view_direction, "scene.viewDirection")?,
            mesh_transforms: &transforms,
        }),
        None => None,
    };
    let modifiers = Modifiers {
        shift: input.modifiers.shift,
        control: input.modifiers.control,
        alt: input.modifiers.alt,
        meta: input.modifiers.meta,
    };
    let input = PointerInput {
        modifiers,
        pick,
        ray,
        scene,
    };

    Ok(editor
        .active_tool_mut()
        .is_some_and(|mut tool| handler(&mut tool, &input)))
}

/// Each picked mesh with its components, owned so `PickHit`s can borrow them.
fn picked_components(mesh_hits: Vec<PickHitJs>) -> Vec<(MeshKey, Vec<ComponentKey>)> {
    mesh_hits
        .into_iter()
        .map(|mesh_hit| {
            (
                MeshKey::from(KeyData::from_ffi(mesh_hit.mesh)),
                mesh_hit.component_keys.into_component_keys(),
            )
        })
        .collect()
}

fn world_ray(ray: RayLike) -> Result<WorldRay, JsError> {
    let origin = DVec3::new(ray.origin.x, ray.origin.y, ray.origin.z);

    if !origin.is_finite() {
        return Err(JsError::new("ray.origin must be finite"));
    }

    Ok(WorldRay {
        origin,
        direction: unit_vector(&ray.direction, "ray.direction")?,
    })
}

fn unit_vector(vector: &Vector3Like, name: &str) -> Result<DVec3, JsError> {
    DVec3::new(vector.x, vector.y, vector.z)
        .try_normalize()
        .ok_or_else(|| JsError::new(&format!("{name} must be finite and non-zero")))
}

fn mesh_transforms(scene: &SceneInputJs) -> Result<Vec<MeshTransform>, JsError> {
    let keys = scene.mesh_keys.to_vec();
    let matrices = scene.matrices.to_vec();

    if matrices.len() != keys.len() * 16 {
        return Err(JsError::new(
            "scene.matrices must hold 16 numbers per entry in scene.meshKeys",
        ));
    }

    keys.iter()
        .zip(matrices.chunks_exact(16))
        .enumerate()
        .map(|(i, (&key, matrix))| {
            let mesh = MeshKey::from(KeyData::from_ffi(key));

            MeshTransform::new(mesh, DMat4::from_cols_slice(matrix)).ok_or_else(|| {
                JsError::new(&format!(
                    "scene.matrices entry {i} must be an invertible affine transform"
                ))
            })
        })
        .collect()
}
