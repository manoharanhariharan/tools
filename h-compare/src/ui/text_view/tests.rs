use super::*;
use crate::load::{OpenData, OpenKind};
use crate::model::CompareOptions;
use crate::textdiff::build_text_diff;
use std::path::PathBuf;

fn data_with(view: crate::textdiff::TextDiffView) -> OpenData {
    OpenData {
        rel: "unicode.txt".to_string(),
        kind: OpenKind::Text,
        left_path: Some(PathBuf::from("A/unicode.txt")),
        right_path: Some(PathBuf::from("B/unicode.txt")),
        left_len: Some(100),
        right_len: Some(100),
        left_modified: None,
        right_modified: None,
        text: Some(view),
        csv: None,
        binary: None,
        image: None,
        messages: vec![],
        error: None,
        first_binary_diff: None,
    }
}

#[test]
fn renders_unicode_and_search_without_panicking() {
    let view = build_text_diff(
        "café → 日本語\nsame line\nold value with many characters\n",
        "cafe → 日本\nsame line\nnew value with many characters\n",
        &CompareOptions::default(),
    );
    let data = data_with(view);
    let mut state = TextUiState::new(data.text.as_ref().unwrap());
    state.search.open = true;
    state.search.query = "日本".to_string();
    state.refresh_search(data.text.as_ref().unwrap());
    let mut options = CompareOptions::default();
    let mut actions = Vec::new();

    let ctx = egui::Context::default();
    for _ in 0..2 {
        let raw = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1400.0, 900.0),
            )),
            ..Default::default()
        };
        let _ = ctx.run(raw, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let reload = show(
                    ui,
                    data.text.as_ref().unwrap(),
                    &mut state,
                    &data,
                    &mut options,
                    &mut actions,
                );
                assert!(!reload);
            });
        });
    }
    assert!(!state.search.matches.is_empty());
}

#[test]
fn text_change_navigation_scrolls_vertically() {
    let mut left = String::new();
    let mut right = String::new();
    for i in 1..=400 {
        left.push_str(&format!("line {i}\n"));
        if i == 300 {
            right.push_str("line 300 changed\n");
        } else {
            right.push_str(&format!("line {i}\n"));
        }
    }
    let view = build_text_diff(&left, &right, &CompareOptions::default());
    let data = data_with(view.clone());
    let mut state = TextUiState::new(&view);
    state.mode = TextViewMode::SideBySide;
    state.current_change = Some(0);
    state.selected_row = Some(0);
    state.scroll_to_row = None;
    state.sync_scroll_y = 0.0;
    let mut options = CompareOptions::default();
    let mut actions = Vec::new();
    let ctx = egui::Context::default();
    let mut frame = |state: &mut TextUiState| {
        let raw = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1400.0, 900.0),
            )),
            ..Default::default()
        };
        let _ = ctx.run(raw, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let _ = show(ui, &view, state, &data, &mut options, &mut actions);
            });
        });
    };
    frame(&mut state);
    assert!(state.sync_scroll_y < 100.0);
    assert!(state.goto_change(&view, true));
    frame(&mut state);
    assert!(
        state.sync_scroll_y > 3000.0,
        "vertical offset after next change was {}",
        state.sync_scroll_y
    );
    frame(&mut state);
    assert!(
        state.sync_scroll_y > 3000.0,
        "vertical offset did not stick: {}",
        state.sync_scroll_y
    );
}

#[test]
fn text_side_by_side_horizontal_scroll_is_shared() {
    let filler = "x".repeat(200);
    let other = "y".repeat(200);
    let mut left = String::new();
    let mut right = String::new();
    for i in 1..=60 {
        left.push_str(&format!("line {i} {filler}\n"));
        right.push_str(&format!("line {i} {other}\n"));
    }
    let view = build_text_diff(&left, &right, &CompareOptions::default());
    let data = data_with(view.clone());
    let mut state = TextUiState::new(&view);
    state.mode = TextViewMode::SideBySide;
    state.scroll_to_row = None;
    state.scroll_to_char = None;
    state.sync_scroll_x = 120.0;
    let mut options = CompareOptions::default();
    let mut actions = Vec::new();
    let ctx = egui::Context::default();
    let mut frame = |state: &mut TextUiState| {
        let raw = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1400.0, 900.0),
            )),
            ..Default::default()
        };
        let _ = ctx.run(raw, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let _ = show(ui, &view, state, &data, &mut options, &mut actions);
            });
        });
    };
    frame(&mut state);
    assert!(
        state.sync_scroll_x > 50.0,
        "horizontal offset after render was {}",
        state.sync_scroll_x
    );
    frame(&mut state);
    assert!(
        state.sync_scroll_x > 50.0,
        "horizontal offset did not stick: {}",
        state.sync_scroll_x
    );
}

#[test]
fn merged_text_mode_renders_and_navigates() {
    let view = build_text_diff(
        "same\nold one\nold two\nsame again\n",
        "same\nnew one\nnew two\nsame again\n",
        &CompareOptions::default(),
    );
    let data = data_with(view.clone());
    let mut state = TextUiState::new(&view);
    state.mode = TextViewMode::Merged;
    let mut options = CompareOptions::default();
    let mut actions = Vec::new();
    let ctx = egui::Context::default();
    for _ in 0..2 {
        let raw = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1400.0, 900.0),
            )),
            ..Default::default()
        };
        let _ = ctx.run(raw, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let _ = show(ui, &view, &mut state, &data, &mut options, &mut actions);
            });
        });
    }
    assert!(state.goto_change(&view, true));
}

#[test]
fn navigation_steps_through_intraline_groups() {
    let view = build_text_diff(
        "one\nline with KLK and lkld end\nthree\n",
        "one\nline with hhh and and end\nthree\n",
        &CompareOptions::default(),
    );
    let row = view.change_rows[0];
    assert_eq!(view.rows[row].change_groups, 2);
    let mut state = TextUiState::new(&view);
    assert_eq!(change_progress(&view, &state), (1, 2));
    assert!(state.goto_change(&view, true));
    assert_eq!(state.selected_group, 1);
    assert_eq!(change_progress(&view, &state), (2, 2));
    assert!(state.goto_change(&view, true));
    assert_eq!(state.selected_group, 0);
    assert_eq!(change_progress(&view, &state), (1, 2));
}

#[test]
fn each_modified_line_is_a_separate_change_target() {
    let view = build_text_diff(
        "one\nline with KLK end\nthree\nfour\nline with ; AND djfjk\nsix\n",
        "one\nline with hhh end\nthree\nfour\nline with ; AND\nsix\n",
        &CompareOptions::default(),
    );
    assert_eq!(view.change_rows, vec![1, 4]);
    assert_eq!(view.stats.modified, 2);
    assert_eq!(view.next_change(Some(1)), Some(4));
    assert_eq!(view.previous_change(Some(4)), Some(1));

    let mut state = TextUiState::new(&view);
    state.current_change = Some(1);
    state.selected_row = Some(1);
    assert!(state.goto_change(&view, true));
    assert_eq!(state.selected_row, Some(4));
    assert_eq!(state.current_change, Some(4));
    assert_eq!(state.scroll_to_row, Some(4));
    // Wraps inside the file instead of jumping to another file.
    assert!(state.goto_change(&view, true));
    assert_eq!(state.selected_row, Some(1));
    assert!(state.goto_change(&view, false));
    assert_eq!(state.selected_row, Some(4));
}

#[test]
fn arrow_navigation_moves_the_text_cursor_by_line_and_group() {
    let view = build_text_diff(
        "one\ntwo\nthree\nfour\n",
        "one\nTWO\nthree\nfour\n",
        &CompareOptions::default(),
    );
    let mut state = TextUiState::new(&view);
    state.selected_row = Some(0);
    state.selected_group = 0;
    state.current_change = None;
    state.scroll_to_row = None;

    assert!(state.navigate(&view, crate::ui::common::NavKey::Down));
    assert_eq!(state.selected_row, Some(1));
    assert_eq!(state.current_change, Some(1));
    assert_eq!(state.scroll_to_row, Some(1));

    assert!(state.navigate(&view, crate::ui::common::NavKey::Down));
    assert_eq!(state.selected_row, Some(2));
    assert_eq!(state.current_change, None);

    assert!(state.navigate(&view, crate::ui::common::NavKey::End));
    assert_eq!(state.selected_row, Some(3));

    assert!(state.navigate(&view, crate::ui::common::NavKey::Home));
    assert_eq!(state.selected_row, Some(0));

    assert!(state.navigate(&view, crate::ui::common::NavKey::PageDown));
    assert_eq!(state.selected_row, Some(3));

    assert!(!state.navigate(&view, crate::ui::common::NavKey::PageDown));
    assert!(state.navigate(&view, crate::ui::common::NavKey::PageUp));
    assert_eq!(state.selected_row, Some(0));
}

#[test]
fn left_and_right_arrows_step_through_change_groups() {
    let view = build_text_diff(
        "one\nline with KLK and lkld end\nthree\n",
        "one\nline with hhh and and end\nthree\n",
        &CompareOptions::default(),
    );
    let mut state = TextUiState::new(&view);
    assert_eq!(state.selected_group, 0);

    assert!(state.navigate(&view, crate::ui::common::NavKey::Right));
    assert_eq!(state.selected_row, Some(1));
    assert_eq!(state.selected_group, 1);

    assert!(state.navigate(&view, crate::ui::common::NavKey::Left));
    assert_eq!(state.selected_group, 0);
}
