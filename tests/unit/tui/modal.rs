use super::*;

use crate::tui::theme;
use ratatui::{backend::TestBackend, buffer::Buffer, style::Modifier, text::Line, Terminal};

// ---- Helpers ----

fn frame_at(x: u16, y: u16, width: u16, height: u16) -> Rect {
    Rect {
        x,
        y,
        width,
        height,
    }
}

fn assert_contained(area: Rect, frame: Rect) {
    assert!(
        area.x >= frame.x && area.y >= frame.y,
        "area {area:?} starts outside frame {frame:?}"
    );
    assert!(
        area.x + area.width <= frame.x + frame.width,
        "area {area:?} overflows frame {frame:?} horizontally"
    );
    assert!(
        area.y + area.height <= frame.y + frame.height,
        "area {area:?} overflows frame {frame:?} vertically"
    );
}

fn assert_centered(area: Rect, frame: Rect) {
    let left_gap = area.x - frame.x;
    let right_gap = (frame.x + frame.width) - (area.x + area.width);
    assert!(
        left_gap.abs_diff(right_gap) <= 1,
        "not centered horizontally: left={left_gap} right={right_gap}"
    );
    let top_gap = area.y - frame.y;
    let bottom_gap = (frame.y + frame.height) - (area.y + area.height);
    assert!(
        top_gap.abs_diff(bottom_gap) <= 1,
        "not centered vertically: top={top_gap} bottom={bottom_gap}"
    );
}

fn empty_content() -> ModalContent {
    ModalContent {
        title: "Compose".to_owned(),
        body: vec![Line::from("Hello")],
        hint: Some("Ctrl+S send · Esc cancel".to_owned()),
        status: None,
        buttons: vec![],
    }
}

fn row_text(buf: &Buffer, row: u16) -> String {
    (0..buf.area.width)
        .map(|col| buf[(col, row)].symbol().to_owned())
        .collect()
}

fn find_text_position(buf: &Buffer, needle: &str) -> Option<(u16, u16)> {
    for row in 0..buf.area.height {
        let text = row_text(buf, row);
        if let Some(start) = text.find(needle) {
            let col = text[..start].chars().count() as u16;
            return Some((col, row));
        }
    }
    None
}

fn render_to_buffer(frame_w: u16, frame_h: u16, content: &ModalContent) -> (Buffer, ModalRender) {
    let backend = TestBackend::new(frame_w, frame_h);
    let mut terminal = Terminal::new(backend).unwrap();
    let mut result = None;
    terminal
        .draw(|frame| {
            let area = frame.area();
            result = Some(render_modal(frame, area, content));
        })
        .unwrap();
    (terminal.backend().buffer().clone(), result.unwrap())
}

// ---- modal_area: pure, contained, centered (AC1) ----

#[test]
fn modal_area_centers_and_fits_within_a_large_frame() {
    let frame = frame_at(0, 0, 120, 40);

    let area = modal_area(frame, 84, 28);

    assert_contained(area, frame);
    assert_centered(area, frame);
    assert_eq!(area.width, 84);
    assert_eq!(area.height, 28);
}

#[test]
fn modal_area_clamps_desired_size_that_exceeds_the_frame() {
    let frame = frame_at(0, 0, 120, 40);

    let area = modal_area(frame, 500, 500);

    assert_contained(area, frame);
    assert_centered(area, frame);
    assert!(area.width < frame.width);
    assert!(area.height < frame.height);
}

#[test]
fn modal_area_centers_within_a_small_frame() {
    let frame = frame_at(0, 0, 20, 10);

    let area = modal_area(frame, 8, 6);

    assert_contained(area, frame);
    assert_centered(area, frame);
    assert_eq!(area, frame_at(6, 2, 8, 6));
}

#[test]
fn modal_area_never_overflows_a_narrow_frame() {
    let frame = frame_at(0, 0, 3, 20);

    let area = modal_area(frame, 50, 10);

    assert_contained(area, frame);
    assert_centered(area, frame);
}

#[test]
fn modal_area_never_overflows_a_degenerate_zero_size_frame() {
    let frame = frame_at(5, 5, 0, 0);

    let area = modal_area(frame, 50, 20);

    assert_contained(area, frame);
    assert_eq!(area.width, 0);
    assert_eq!(area.height, 0);
}

#[test]
fn modal_area_never_overflows_across_a_sweep_of_frame_and_desired_sizes() {
    for width in 0..24u16 {
        for height in 0..24u16 {
            let frame = frame_at(0, 0, width, height);
            for desired in [0u16, 1, 5, 40, 200] {
                let area = modal_area(frame, desired, desired);
                assert_contained(area, frame);
                assert_centered(area, frame);
            }
        }
    }
}

#[test]
fn modal_area_is_pure_and_offset_aware() {
    let frame = frame_at(10, 20, 40, 20);

    let area = modal_area(frame, 20, 10);

    assert_contained(area, frame);
    assert_centered(area, frame);
    assert!(area.x >= 10 && area.y >= 20);
}

// ---- render_modal: backdrop dim, centered box, title/hint/body (AC2) ----

#[test]
fn render_modal_dims_the_backdrop_outside_the_box() {
    let content = empty_content();

    let (buf, render) = render_to_buffer(60, 24, &content);

    assert!(render.area.x > 0, "box should not touch the left edge");
    let backdrop_cell = &buf[(0, 0)];
    let style = backdrop_cell.style();
    assert!(
        style.add_modifier.contains(Modifier::DIM),
        "backdrop cell must carry DIM: {style:?}"
    );
    assert_eq!(style.bg, theme::modal_backdrop().bg);
}

#[test]
fn render_modal_boxes_at_roughly_seventy_percent_of_the_frame() {
    let content = empty_content();
    let frame = frame_at(0, 0, 60, 24);

    let (_buf, render) = render_to_buffer(60, 24, &content);

    assert_contained(render.area, frame);
    assert_centered(render.area, frame);
    let expected_w = (frame.width as u32 * 70 / 100) as u16;
    let expected_h = (frame.height as u32 * 70 / 100) as u16;
    assert_eq!(render.area.width, expected_w);
    assert_eq!(render.area.height, expected_h);
}

#[test]
fn render_modal_draws_title_hint_and_body_inside_the_box() {
    let content = empty_content();

    let (buf, _render) = render_to_buffer(60, 24, &content);

    assert!(
        find_text_position(&buf, "Compose").is_some(),
        "title must render"
    );
    assert!(
        find_text_position(&buf, "Ctrl+S send").is_some(),
        "hint must render"
    );
    assert!(
        find_text_position(&buf, "Hello").is_some(),
        "body line must render"
    );
}

#[test]
fn render_modal_draws_the_status_line_when_present() {
    let mut content = empty_content();
    content.status = Some("Submitting…".to_owned());

    let (buf, _render) = render_to_buffer(60, 24, &content);

    assert!(
        find_text_position(&buf, "Submitting").is_some(),
        "status must render inside the box"
    );
}

#[test]
fn render_modal_never_panics_without_hint_or_status_or_buttons() {
    let content = ModalContent {
        title: "Confirm".to_owned(),
        body: vec![Line::from("Delete this comment?")],
        hint: None,
        status: None,
        buttons: vec![],
    };

    let (_buf, render) = render_to_buffer(40, 12, &content);

    assert!(render.buttons.is_empty());
}

// ---- render_modal: buttons registered in modal-relative coordinates (AC3) ----

#[test]
fn render_modal_registers_button_click_targets_in_modal_relative_coordinates() {
    let content = ModalContent {
        title: "Confirm".to_owned(),
        body: vec![Line::from("Sure?")],
        hint: None,
        status: None,
        buttons: vec![
            ModalButton {
                id: "confirm".to_owned(),
                label: "Delete".to_owned(),
            },
            ModalButton {
                id: "cancel".to_owned(),
                label: "Cancel".to_owned(),
            },
        ],
    };

    let (buf, render) = render_to_buffer(60, 24, &content);

    assert_eq!(render.buttons.len(), 2);
    assert_eq!(render.buttons[0].id, "confirm");
    assert_eq!(render.buttons[1].id, "cancel");

    for button in &render.buttons {
        assert!(button.area.x < render.area.width);
        assert!(button.area.y < render.area.height);
    }

    let (delete_col, delete_row) =
        find_text_position(&buf, "[ Delete ]").expect("delete button label must render");
    let expected = &render.buttons[0];
    assert_eq!(delete_row, render.area.y + expected.area.y);
    assert_eq!(delete_col, render.area.x + expected.area.x);
}

// ---- button_targets: pure ABSOLUTE-coordinate geometry, the single source
// render_buttons AND a caller's hit-test both build on (ADR 0024 §2d) ----

fn two_button_content() -> ModalContent {
    ModalContent {
        title: "Confirm".to_owned(),
        body: vec![Line::from("Sure?")],
        hint: None,
        status: None,
        buttons: vec![
            ModalButton {
                id: "yes".to_owned(),
                label: "Yes".to_owned(),
            },
            ModalButton {
                id: "no".to_owned(),
                label: "No".to_owned(),
            },
        ],
    }
}

#[test]
fn button_targets_returns_two_targets_on_the_same_row_advancing_by_x() {
    let content = two_button_content();
    let frame = frame_at(0, 0, 60, 24);

    let targets = button_targets(frame, &content);

    assert_eq!(targets.len(), 2);
    assert_eq!(targets[0].id, "yes");
    assert_eq!(targets[1].id, "no");
    assert_eq!(
        targets[0].area.y, targets[1].area.y,
        "both buttons must sit on the same (buttons) row"
    );
    assert!(
        targets[1].area.x > targets[0].area.x + targets[0].area.width,
        "the second button's x ({}) must advance beyond the first's x+width ({})",
        targets[1].area.x,
        targets[0].area.x + targets[0].area.width
    );
}

#[test]
fn button_targets_areas_sit_inside_the_modal_box() {
    let content = two_button_content();
    let frame = frame_at(0, 0, 60, 24);

    let targets = button_targets(frame, &content);
    let (desired_w, desired_h) = desired_size(frame, &content);
    let modal_box = modal_area(frame, desired_w, desired_h);

    for target in &targets {
        assert_contained(target.area, modal_box);
    }
}

#[test]
fn button_targets_matches_the_absolute_coordinates_render_modal_draws_at() {
    let content = two_button_content();

    let (buf, render) = render_to_buffer(60, 24, &content);
    let targets = button_targets(frame_at(0, 0, 60, 24), &content);

    let (yes_col, yes_row) =
        find_text_position(&buf, "[ Yes ]").expect("yes button label must render");
    let yes_target = &targets[0];
    assert_eq!(yes_row, yes_target.area.y);
    assert_eq!(yes_col, yes_target.area.x);

    // The SAME geometry, expressed modal-relative by render_modal's own
    // ModalRender, must recover the identical absolute coordinate.
    let relative_yes = &render.buttons[0];
    assert_eq!(render.area.x + relative_yes.area.x, yes_target.area.x);
    assert_eq!(render.area.y + relative_yes.area.y, yes_target.area.y);
}
