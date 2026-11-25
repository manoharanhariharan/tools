use super::*;
use crate::csvdiff::compare_csv;
use crate::load::{OpenData, OpenKind};
use std::path::PathBuf;

fn data_with(compare: CsvCompare) -> OpenData {
    OpenData {
        rel: "data.csv".to_string(),
        kind: OpenKind::Csv,
        left_path: Some(PathBuf::from("A/data.csv")),
        right_path: Some(PathBuf::from("B/data.csv")),
        left_len: Some(100),
        right_len: Some(100),
        left_modified: None,
        right_modified: None,
        text: None,
        csv: Some(compare),
        messages: vec![],
        error: None,
        first_binary_diff: None,
    }
}

#[test]
fn renders_merged_and_side_by_side_without_panicking() {
    let compare = compare_csv(
        "id,name,note\n1,alpha,café\n2,beta,\"line 1\nline 2\"\n3,gamma,third\n".as_bytes(),
        "id,name,note\n1,alpha,cafe\n2,BETA,\"line 1\nline 3\"\n4,delta,fourth\n".as_bytes(),
        &CsvOptions::default(),
    )
    .unwrap();
    let data = data_with(compare);
    let mut state = CsvUiState::new(data.csv.as_ref().unwrap());
    state.hidden_columns.insert(2);
    assert_eq!(
        state.visible_columns(data.csv.as_ref().unwrap()),
        vec![0, 1]
    );
    state.search.open = true;
    state.search.query = "beta".to_string();
    state.refresh_search(data.csv.as_ref().unwrap());
    let mut options = CsvOptions::default();
    let mut actions = Vec::new();

    for mode in [CsvViewMode::Merged, CsvViewMode::SideBySide] {
        state.mode = mode;
        state.differences_only = mode == CsvViewMode::SideBySide;
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
                        data.csv.as_ref().unwrap(),
                        &mut state,
                        &data,
                        &mut options,
                        &mut actions,
                        false,
                    );
                    assert!(!reload);
                });
            });
        }
    }
    assert!(!state.search.matches.is_empty());
    actions.clear();
    state.hidden_dirty = true;
    let ctx = egui::Context::default();
    let raw = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(1400.0, 900.0),
        )),
        ..Default::default()
    };
    let _ = ctx.run(raw, |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| {
            let _ = show(
                ui,
                data.csv.as_ref().unwrap(),
                &mut state,
                &data,
                &mut options,
                &mut actions,
                false,
            );
        });
    });
    assert!(actions
        .iter()
        .any(|action| matches!(action, UiAction::RecomputeCsv { .. })));
}

#[test]
fn goto_change_focuses_the_changed_cell() {
    let compare = compare_csv(
        b"id,name,note\n1,alpha,keep\n2,beta,old\n3,gamma,stable\n",
        b"id,name,note\n1,alpha,keep\n2,beta,new\n3,gamma,stable\n",
        &CsvOptions::default(),
    )
    .unwrap();
    let mut state = CsvUiState::new(&compare);
    state.current_change = Some(0);
    state.selected_cell = None;
    assert!(state.goto_change(&compare, true));
    let row = state.current_change.expect("a change row");
    assert_eq!(state.selected_row, Some(row));
    assert_eq!(state.selected_cell, Some((row, 2)));
    assert_eq!(state.scroll_to_col, Some(2));
}

#[test]
fn goto_change_steps_through_cells_before_the_next_row() {
    let compare = compare_csv(
        b"id,a,b,c\n1,x,y,z\n2,p,q,r\n",
        b"id,a,b,c\n1,X,Y,z\n2,p,q,r\n",
        &CsvOptions::default(),
    )
    .unwrap();
    let mut state = CsvUiState::new(&compare);
    let row = state.current_change.expect("change row");
    assert_eq!(state.selected_cell, Some((row, 1)));
    assert_eq!(change_progress(&compare, &state), (1, 2));
    assert!(state.goto_change(&compare, true));
    assert_eq!(state.selected_cell, Some((row, 2)));
    assert_eq!(change_progress(&compare, &state), (2, 2));
    assert!(state.goto_change(&compare, true));
    assert_eq!(state.selected_cell, Some((row, 1)));
    assert!(state.goto_change(&compare, false));
    assert_eq!(state.selected_cell, Some((row, 2)));
}

#[test]
fn goto_change_wraps_within_the_file_instead_of_skipping() {
    let compare = compare_csv(
        b"id,name\n1,alpha\n2,beta\n",
        b"id,name\n1,alpha\n2,BETA\n",
        &CsvOptions::default(),
    )
    .unwrap();
    let mut state = CsvUiState::new(&compare);
    let first = state.current_change.expect("first change");
    assert!(state.goto_change(&compare, true));
    assert_eq!(state.current_change, Some(first));
    assert_eq!(state.selected_cell, Some((first, 1)));
    assert!(state.goto_change(&compare, false));
    assert_eq!(state.current_change, Some(first));
}

#[test]
fn change_navigation_scrolls_horizontally_and_keeps_the_offset() {
    let mut left = String::from("id");
    let mut right = String::from("id");
    for c in 1..=20 {
        left.push_str(&format!(",c{c}"));
        right.push_str(&format!(",c{c}"));
    }
    left.push('\n');
    right.push('\n');
    for r in 1..=15 {
        let mut left_row = format!("{r}");
        let mut right_row = format!("{r}");
        for c in 1..=20 {
            let original = format!("r{r}c{c}");
            left_row.push_str(&format!(",{original}"));
            if r == 6 && c == 17 {
                right_row.push_str(",CHANGED");
            } else {
                right_row.push_str(&format!(",{original}"));
            }
        }
        left.push_str(&left_row);
        left.push('\n');
        right.push_str(&right_row);
        right.push('\n');
    }
    let compare = compare_csv(left.as_bytes(), right.as_bytes(), &CsvOptions::default()).unwrap();
    assert_eq!(compare.align, CsvAlign::Key(0));
    let data = data_with(compare.clone());
    let mut state = CsvUiState::new(&compare);
    state.mode = CsvViewMode::SideBySide;
    let mut options = CsvOptions::default();
    let mut actions = Vec::new();
    let ctx = egui::Context::default();
    let mut frame = |state: &mut CsvUiState| {
        let raw = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1400.0, 900.0),
            )),
            ..Default::default()
        };
        let _ = ctx.run(raw, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let _ = show(
                    ui,
                    &compare,
                    state,
                    &data,
                    &mut options,
                    &mut actions,
                    false,
                );
            });
        });
    };
    frame(&mut state);
    assert!(
        state.sync_scroll_x > 100.0,
        "initial focus offset was {}",
        state.sync_scroll_x
    );
    state.sync_scroll_x = 0.0;
    assert!(state.goto_change(&compare, true));
    frame(&mut state);
    assert!(
        state.sync_scroll_x > 100.0,
        "offset after next change was {}",
        state.sync_scroll_x
    );
    frame(&mut state);
    assert!(
        state.sync_scroll_x > 100.0,
        "offset did not stick: {}",
        state.sync_scroll_x
    );
}

#[test]
fn side_by_side_change_navigation_scrolls_vertically_to_the_row() {
    let mut left = String::from("id,value\n");
    let mut right = String::from("id,value\n");
    for i in 1..=400 {
        left.push_str(&format!("{i},v{i}\n"));
        if i == 300 {
            right.push_str(&format!("{i},CHANGED\n"));
        } else {
            right.push_str(&format!("{i},v{i}\n"));
        }
    }
    let compare = compare_csv(left.as_bytes(), right.as_bytes(), &CsvOptions::default()).unwrap();
    let data = data_with(compare.clone());
    let mut state = CsvUiState::new(&compare);
    state.mode = CsvViewMode::SideBySide;
    state.current_change = Some(0);
    state.selected_row = Some(0);
    state.selected_cell = Some((0, 0));
    state.scroll_to_row = None;
    state.scroll_to_col = None;
    state.focus_frames = 0;
    state.sync_scroll_y = 0.0;
    let mut options = CsvOptions::default();
    let mut actions = Vec::new();
    let ctx = egui::Context::default();
    let mut frame = |state: &mut CsvUiState| {
        let raw = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1400.0, 900.0),
            )),
            ..Default::default()
        };
        let _ = ctx.run(raw, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let _ = show(
                    ui,
                    &compare,
                    state,
                    &data,
                    &mut options,
                    &mut actions,
                    false,
                );
            });
        });
    };
    frame(&mut state);
    assert!(state.sync_scroll_y < 100.0);
    assert!(state.goto_change(&compare, true));
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
fn column_visibility_entries_list_visible_first_then_hidden() {
    let compare = compare_csv(
        b"id,a,b,c\n1,x,y,z\n",
        b"id,a,b,c\n1,x,y,z\n",
        &CsvOptions::default(),
    )
    .unwrap();
    let mut state = CsvUiState::new(&compare);
    state.hidden_columns.insert(1);
    state.hidden_columns.insert(3);
    let (visible, hidden_cols) = state.column_visibility_entries(&compare);
    assert_eq!(visible, vec![0, 2]);
    assert_eq!(hidden_cols, vec![1, 3]);

    state.set_column_visible(&compare, 1, true);
    assert!(!state.hidden_columns.contains(&1));
    state.set_column_visible(&compare, 0, false);
    let (visible, hidden_cols) = state.column_visibility_entries(&compare);
    assert_eq!(visible, vec![1, 2]);
    assert_eq!(hidden_cols, vec![0, 3]);
    assert_eq!(visible.len() + hidden_cols.len(), compare.column_count);
}

#[test]
fn differences_only_hides_unchanged_columns_and_allows_override() {
    let compare = compare_csv(
        b"id,a,b,c\n1,x,y,z\n",
        b"id,a,b,c\n1,X,y,z\n",
        &CsvOptions::default(),
    )
    .unwrap();
    let mut state = CsvUiState::new(&compare);
    state.differences_only = true;
    assert!(state.column_visible(&compare, 1));
    assert!(!state.column_visible(&compare, 0));
    assert!(!state.column_visible(&compare, 2));
    let (visible, hidden_cols) = state.column_visibility_entries(&compare);
    assert_eq!(visible, vec![1]);
    assert_eq!(hidden_cols, vec![0, 2, 3]);

    // Explicitly show an unchanged column; differences-only must respect it.
    state.set_column_visible(&compare, 0, true);
    assert!(state.column_visible(&compare, 0));
    let (visible, hidden_cols) = state.column_visibility_entries(&compare);
    assert_eq!(visible, vec![0, 1]);
    assert_eq!(hidden_cols, vec![2, 3]);

    // Hiding it again returns it to the auto-hidden group.
    state.set_column_visible(&compare, 0, false);
    assert!(!state.column_visible(&compare, 0));
    assert!(state.shown_columns.is_empty());

    // Turning differences-only off restores auto-hidden columns; only explicit
    // hides remain hidden.
    state.differences_only = false;
    assert!(state.column_visible(&compare, 0));
    assert!(state.column_visible(&compare, 2));
}

#[test]
fn column_header_tooltip_reports_cell_and_header_differences() {
    let compare = compare_csv(
        b"id,name,note\n1,alpha,keep\n",
        b"id,title,note\n1,alpha,CHANGED\n",
        &CsvOptions::default(),
    )
    .unwrap();
    let header_tip = super::menus::column_header_tooltip(&compare, 1).expect("header tooltip");
    assert!(header_tip.contains("Header differs"));
    assert!(header_tip.contains("Left: name"));
    assert!(header_tip.contains("Right: title"));
    assert!(!header_tip.contains("differing cell"));

    let cell_tip = super::menus::column_header_tooltip(&compare, 2).expect("cell tooltip");
    assert!(cell_tip.contains("1 differing cell in this column"));
    assert!(!cell_tip.contains("Header differs"));

    assert!(super::menus::column_header_tooltip(&compare, 0).is_none());
}

#[test]
fn huge_values_are_capped_for_rendering() {
    let value = "x".repeat(5000);
    let shown = display_value_capped(&value, RENDER_VALUE_CHARS);
    assert_eq!(shown.chars().count(), RENDER_VALUE_CHARS + 1);
    assert!(shown.ends_with('…'));
    assert_eq!(
        display_value_capped("short", RENDER_VALUE_CHARS).as_ref(),
        "short"
    );
}

#[test]
fn arrow_navigation_moves_between_visible_cells() {
    let compare = compare_csv(
        b"id,a,b\n1,x,y\n2,p,q\n3,s,t\n",
        b"id,a,b\n1,x,y\n2,p,q\n3,s,t\n",
        &CsvOptions::default(),
    )
    .unwrap();
    let mut state = CsvUiState::new(&compare);
    state.selected_row = Some(0);
    state.selected_cell = Some((0, 0));
    state.scroll_to_col = None;
    let visible: Vec<usize> = (0..compare.rows.len()).collect();

    assert!(state.navigate_cells(&compare, &visible, crate::ui::common::NavKey::Right));
    assert_eq!(state.selected_cell, Some((0, 1)));
    assert_eq!(state.scroll_to_col, Some(1));

    assert!(state.navigate_cells(&compare, &visible, crate::ui::common::NavKey::Down));
    assert_eq!(state.selected_cell, Some((1, 1)));
    assert_eq!(state.selected_row, Some(1));

    assert!(state.navigate_cells(&compare, &visible, crate::ui::common::NavKey::Left));
    assert_eq!(state.selected_cell, Some((1, 0)));

    assert!(state.navigate_cells(&compare, &visible, crate::ui::common::NavKey::End));
    assert_eq!(state.selected_cell, Some((1, 2)));

    assert!(state.navigate_cells(&compare, &visible, crate::ui::common::NavKey::Home));
    assert_eq!(state.selected_cell, Some((1, 0)));
}

#[test]
fn arrow_navigation_skips_hidden_columns_and_filtered_rows() {
    let compare = compare_csv(
        b"id,a,b\n1,x,y\n2,p,q\n3,s,t\n",
        b"id,a,b\n1,x,y\n2,p,q\n3,s,t\n",
        &CsvOptions::default(),
    )
    .unwrap();
    let mut state = CsvUiState::new(&compare);
    state.hidden_columns.insert(1);
    state.selected_row = Some(0);
    state.selected_cell = Some((0, 0));
    let visible = vec![1, 2];

    assert!(state.navigate_cells(&compare, &visible, crate::ui::common::NavKey::Right));
    assert_eq!(state.selected_cell, Some((1, 2)));
    assert!(!state.navigate_cells(&compare, &visible, crate::ui::common::NavKey::Right));
    assert_eq!(state.selected_cell, Some((1, 2)));

    assert!(state.navigate_cells(&compare, &visible, crate::ui::common::NavKey::Down));
    assert_eq!(state.selected_cell, Some((2, 2)));
    assert!(!state.navigate_cells(&compare, &visible, crate::ui::common::NavKey::Down));
    assert_eq!(state.selected_cell, Some((2, 2)));
}
