use glam::DQuat;

use crate::MeshEditor;
use mesh::Mesh;

use super::*;

#[test]
fn constructor_rejects_invalid_placements() {
    let key = MeshEditor::new().add_mesh(Mesh::new());
    let mut projective = DMat4::IDENTITY;
    projective.x_axis.w = 1.0;

    for matrix in [
        DMat4::ZERO,
        DMat4::from_scale(DVec3::new(0.0, 1.0, 1.0)),
        DMat4::from_scale(DVec3::splat(f64::INFINITY)),
        DMat4::from_scale(DVec3::splat(f64::NAN)),
        projective,
    ] {
        assert_eq!(MeshTransform::new(key, matrix), None);
    }
}

#[test]
fn cached_inverse_maps_vectors_without_translation() {
    let key = MeshEditor::new().add_mesh(Mesh::new());
    let matrix = DMat4::from_scale_rotation_translation(
        DVec3::new(-2.0, 3.0, 4.0),
        DQuat::from_rotation_z(std::f64::consts::FRAC_PI_2),
        DVec3::new(6_378_137.125, -50.0, 20.0),
    );
    let transform = MeshTransform::new(key, matrix).unwrap();
    assert_eq!(transform.mesh, key);
    assert_eq!(transform.local_to_world(), matrix);
    assert_eq!(transform.world_to_local(), matrix.inverse());
    assert!(
        transform
            .transform_world_vec(DVec3::new(6.0, 4.0, 0.0))
            .abs_diff_eq(Vec3::new(-2.0, -2.0, 0.0), 1e-5)
    );
}

#[test]
fn small_scale_preserves_representable_local_movements() {
    let key = MeshEditor::new().add_mesh(Mesh::new());
    let transform =
        MeshTransform::new(key, DMat4::from_scale(DVec3::new(1e-40, 1.0, 1.0))).unwrap();

    assert_eq!(transform.transform_world_vec(1e-40 * DVec3::X), Vec3::X);
}

#[test]
#[cfg(debug_assertions)]
#[should_panic(expected = "world vector must be representable in local f32 coordinates")]
fn unrepresentable_local_vector_violates_the_debug_contract() {
    let key = MeshEditor::new().add_mesh(Mesh::new());
    let transform =
        MeshTransform::new(key, DMat4::from_scale(DVec3::new(1e-40, 1.0, 1.0))).unwrap();
    transform.transform_world_vec(DVec3::X);
}

#[test]
#[cfg(debug_assertions)]
#[should_panic(expected = "world vector must be representable in local f32 coordinates")]
fn non_finite_world_vector_violates_the_debug_contract() {
    let key = MeshEditor::new().add_mesh(Mesh::new());
    let transform = MeshTransform::new(key, DMat4::IDENTITY).unwrap();
    transform.transform_world_vec(DVec3::NAN);
}
