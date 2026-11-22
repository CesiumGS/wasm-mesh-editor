use mesh::ComponentMask;
use slotmap::Key;

use super::*;

fn meshes<const COUNT: usize>() -> (SlotMap<MeshKey, Mesh>, [MeshKey; COUNT]) {
    let mut meshes = SlotMap::with_key();
    let keys = std::array::from_fn(|_| meshes.insert(Mesh::new()));

    (meshes, keys)
}

fn take_changes(active: &mut ActiveMeshes) -> (Vec<MeshKey>, Vec<MeshKey>) {
    let mut activated = Vec::new();
    let mut deactivated = Vec::new();

    active.take_changes(&mut activated, &mut deactivated);

    (activated, deactivated)
}

#[test]
fn seeded_meshes_are_active_without_recorded_changes() {
    let (_, [first, second]) = meshes();
    let mut active = ActiveMeshes::from(vec![second, first]);

    assert_eq!(active.keys(), &[second, first]);
    assert!(active.contains(first));
    assert_eq!(take_changes(&mut active), (vec![], vec![]));
}

#[test]
fn activation_applies_selection_level_and_records_once() {
    let (mut meshes, [first, second]) = meshes();
    let mut active = ActiveMeshes::default();

    active
        .activate(&mut meshes, EditMode::EDGE, second)
        .unwrap();
    active.activate(&mut meshes, EditMode::EDGE, first).unwrap();
    meshes[second]
        .selection_mut()
        .set_level(ComponentMask::FACE);
    active
        .activate(&mut meshes, EditMode::VERTEX, second)
        .unwrap();

    assert_eq!(active.keys(), &[second, first]);
    assert_eq!(meshes[first].selection().level(), ComponentMask::EDGE);
    assert_eq!(meshes[second].selection().level(), ComponentMask::FACE);
    assert_eq!(take_changes(&mut active), (vec![second, first], vec![]));
}

#[test]
fn activation_rejects_unknown_keys_without_recording() {
    let (mut meshes, [removed]) = meshes();
    let mut active = ActiveMeshes::default();

    meshes.remove(removed).unwrap();

    for key in [removed, MeshKey::null()] {
        assert_eq!(
            active.activate(&mut meshes, EditMode::VERTEX, key),
            Err(MeshNotFound { key })
        );
    }

    assert!(active.keys().is_empty());
    assert_eq!(take_changes(&mut active), (vec![], vec![]));
}

#[test]
fn deactivation_preserves_order_and_records_only_active_meshes() {
    let (_, [first, middle, last, inactive]) = meshes();
    let mut active = ActiveMeshes::from(vec![first, middle, last]);

    assert!(active.deactivate(middle));
    assert!(!active.deactivate(middle));
    assert!(!active.deactivate(inactive));

    assert_eq!(active.keys(), &[first, last]);
    assert_eq!(take_changes(&mut active), (vec![], vec![middle]));
}

#[test]
fn retain_records_each_removed_mesh_in_order() {
    let (_, [first, middle, last]) = meshes();
    let mut active = ActiveMeshes::from(vec![last, middle, first]);

    active.retain(|&key| key == middle);

    assert_eq!(active.keys(), &[middle]);
    assert_eq!(take_changes(&mut active), (vec![], vec![last, first]));
}

#[test]
fn opposite_changes_before_take_cancel_out() {
    let (mut meshes, [seeded, added, toggled]) = meshes();
    let mut active = ActiveMeshes::from(vec![seeded]);

    active
        .activate(&mut meshes, EditMode::VERTEX, added)
        .unwrap();
    active
        .activate(&mut meshes, EditMode::VERTEX, toggled)
        .unwrap();
    active.deactivate(added);
    active.retain(|&key| key != seeded);
    active
        .activate(&mut meshes, EditMode::VERTEX, seeded)
        .unwrap();
    active.deactivate(toggled);
    active
        .activate(&mut meshes, EditMode::VERTEX, toggled)
        .unwrap();

    assert_eq!(active.keys(), &[seeded, toggled]);
    assert_eq!(take_changes(&mut active), (vec![toggled], vec![]));
}

#[test]
fn take_changes_replaces_lists_by_swapping_buffers() {
    let (mut meshes, [first, second, stale]) = meshes();
    let mut active = ActiveMeshes::from(vec![first]);
    let mut activated = Vec::with_capacity(4);
    let mut deactivated = Vec::with_capacity(4);

    activated.push(stale);
    deactivated.push(stale);

    let activated_allocation = activated.as_ptr();
    let deactivated_allocation = deactivated.as_ptr();

    active
        .activate(&mut meshes, EditMode::VERTEX, second)
        .unwrap();
    active.deactivate(first);
    active.take_changes(&mut activated, &mut deactivated);

    assert_eq!(activated, [second]);
    assert_eq!(deactivated, [first]);

    // The caller's cleared buffers now record changes and are handed back on the next take.
    active.deactivate(second);
    active.take_changes(&mut activated, &mut deactivated);

    assert!(activated.is_empty());
    assert_eq!(deactivated, [second]);
    assert_eq!(activated.as_ptr(), activated_allocation);
    assert_eq!(deactivated.as_ptr(), deactivated_allocation);
    assert_eq!(take_changes(&mut active), (vec![], vec![]));
}
