use super::*;

#[test]
fn drag_positions_start_at_the_press_and_reads_do_not_consume_them() {
    let mut drag = DragState::new(DVec3::new(10.0, 20.0, 0.0));
    assert_eq!(drag.current_position, drag.start_position);
    assert_eq!(drag.last_update_position, drag.start_position);

    drag.update_position(Some(DVec3::new(11.0, 22.0, 0.0)));
    drag.update_position(Some(DVec3::new(13.0, 24.0, 0.0)));
    assert_eq!(drag.current_position, DVec3::new(13.0, 24.0, 0.0));
    assert_eq!(drag.current_position, DVec3::new(13.0, 24.0, 0.0));
    assert_eq!(drag.start_position, DVec3::new(10.0, 20.0, 0.0));
    assert_eq!(drag.last_update_position, drag.start_position);

    drag.update_position(Some(DVec3::new(9.0, 19.0, 0.0)));
    assert_eq!(drag.current_position, DVec3::new(9.0, 19.0, 0.0));
}

#[test]
fn failed_projection_preserves_the_last_valid_position() {
    let mut drag = DragState::new(DVec3::X);
    drag.update_position(None);
    assert_eq!(drag.current_position, DVec3::X);
    drag.update_position(Some(2.0 * DVec3::X));
    drag.update_position(None);
    assert_eq!(drag.current_position, 2.0 * DVec3::X);
    assert_eq!(drag.start_position, DVec3::X);

    drag.update_position(Some(4.0 * DVec3::X));
    assert_eq!(drag.current_position, 4.0 * DVec3::X);
}

#[test]
fn update_advances_only_the_last_update_position_and_returns_incremental_movement() {
    let start = DVec3::new(10.0, 20.0, 30.0);
    let mut drag = DragState::new(start);
    assert_eq!(drag.update(), DVec3::ZERO);

    drag.update_position(Some(start + DVec3::new(1.0, 2.0, 3.0)));
    drag.update_position(Some(start + DVec3::new(4.0, 5.0, 6.0)));
    assert_eq!(drag.update(), DVec3::new(4.0, 5.0, 6.0));
    assert_eq!(drag.start_position, start);
    assert_eq!(drag.current_position, start + DVec3::new(4.0, 5.0, 6.0));
    assert_eq!(drag.last_update_position, drag.current_position);
    assert_eq!(drag.update(), DVec3::ZERO);

    drag.update_position(None);
    assert_eq!(drag.update(), DVec3::ZERO);
    drag.update_position(Some(start));
    assert_eq!(drag.update(), DVec3::new(-4.0, -5.0, -6.0));
    assert_eq!(drag.last_update_position, start);
}
