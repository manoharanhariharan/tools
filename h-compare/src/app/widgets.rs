//! Small widgets and panels shared inside the application layer.

use super::*;

pub(super) struct PathRowResult {
    pub(super) browse: bool,
    pub(super) enter: bool,
}

pub(super) fn path_field(
    ui: &mut Ui,
    badge: &str,
    color: Color32,
    value: &mut String,
    hint: &str,
) -> PathRowResult {
    let mut result = PathRowResult {
        browse: false,
        enter: false,
    };
    ui.horizontal(|ui| {
        Frame::NONE
            .fill(color.gamma_multiply(0.18))
            .stroke(Stroke::new(1.0_f32, color.gamma_multiply(0.5)))
            .corner_radius(4.0)
            .inner_margin(Margin::symmetric(7, 1))
            .show(ui, |ui| {
                ui.label(RichText::new(badge).color(color).strong().size(11.0));
            });
        let width = (ui.available_width() - 118.0).max(160.0);
        let response = ui.add_sized(
            [width, 26.0],
            TextEdit::singleline(value).hint_text(hint).clip_text(true),
        );
        if response.lost_focus() && ui.input(|input| input.key_pressed(Key::Enter)) {
            result.enter = true;
        }
        if common::icon_button(ui, Icon::Folder, "Browse", common::ButtonIntent::Secondary)
            .clicked()
        {
            result.browse = true;
        }
    });
    result
}

pub(super) fn colored_count(ui: &mut Ui, label: &str, count: usize, color: Color32) {
    ui.label(
        RichText::new(format!("● {label} {count}"))
            .color(color)
            .size(11.0),
    );
}

pub(super) fn path_card(ui: &mut Ui, badge: &str, color: Color32, path: &Path, detail: &str) {
    Frame::NONE
        .fill(theme::panel())
        .stroke(Stroke::new(1.0_f32, theme::border()))
        .corner_radius(6.0)
        .inner_margin(Margin::symmetric(10, 5))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new(badge).color(color).strong());
                ui.add(
                    Label::new(
                        RichText::new(path.display().to_string())
                            .color(theme::text())
                            .monospace()
                            .size(11.5),
                    )
                    .truncate(),
                );
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    common::small_muted(ui, detail);
                });
            });
        });
}

pub(super) fn binary_panel(ui: &mut Ui, data: &OpenData, actions: &mut Vec<UiAction>) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(&data.rel).size(20.0).strong());
        ui.label(
            RichText::new("BINARY")
                .color(theme::type_color())
                .size(10.0)
                .strong(),
        );
    });
    ui.add_space(4.0);
    if let Some(offset) = data.first_binary_diff {
        ui.label(
            RichText::new(format!("First difference at byte {offset} (0x{offset:X})."))
                .color(theme::diff()),
        );
    } else if data.left_path.is_some() && data.right_path.is_some() {
        ui.label(RichText::new("Files are byte-for-byte identical.").color(theme::same()));
    }
    if data.left_path.is_none() || data.right_path.is_none() {
        ui.label(
            RichText::new("This entry exists on only one side.")
                .color(theme::muted())
                .size(12.0),
        );
    }
    ui.add_space(10.0);
    ui.columns(2, |columns| {
        binary_side(&mut columns[0], "A", theme::accent(), data, true, actions);
        binary_side(&mut columns[1], "B", theme::right(), data, false, actions);
    });
    ui.add_space(10.0);
    common::small_muted(
        ui,
        "Binary files are compared by metadata and first differing byte. Use your normal binary diff tool for detailed inspection.",
    );
}

pub(super) fn binary_side(
    ui: &mut Ui,
    badge: &str,
    color: Color32,
    data: &OpenData,
    left: bool,
    actions: &mut Vec<UiAction>,
) {
    let path = if left {
        &data.left_path
    } else {
        &data.right_path
    };
    let len = if left { data.left_len } else { data.right_len };
    let modified = if left {
        data.left_modified
    } else {
        data.right_modified
    };
    Frame::NONE
        .fill(theme::panel())
        .stroke(Stroke::new(1.0_f32, theme::border()))
        .corner_radius(8.0)
        .inner_margin(Margin::symmetric(12, 10))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new(badge).color(color).size(16.0).strong());
                ui.label(
                    RichText::new(
                        path.as_deref()
                            .and_then(Path::file_name)
                            .map(|name| name.to_string_lossy().into_owned())
                            .unwrap_or_else(|| "not present".to_string()),
                    )
                    .strong(),
                );
            });
            if let Some(path) = path {
                ui.add(
                    Label::new(
                        RichText::new(path.display().to_string())
                            .color(theme::muted())
                            .monospace()
                            .size(10.5),
                    )
                    .truncate(),
                );
                ui.add_space(4.0);
                ui.label(
                    RichText::new(format!(
                        "{} · modified {}",
                        len.map(format_size).unwrap_or_else(|| "?".to_string()),
                        format_modified(modified)
                    ))
                    .color(theme::muted())
                    .size(11.0),
                );
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    if common::icon_button(ui, Icon::External, "Open", common::ButtonIntent::Ghost)
                        .clicked()
                    {
                        actions.push(UiAction::OpenDefault {
                            rel: data.rel.clone(),
                            left,
                        });
                    }
                    if common::icon_button(ui, Icon::Folder, "Reveal", common::ButtonIntent::Ghost)
                        .clicked()
                    {
                        actions.push(UiAction::Reveal {
                            rel: data.rel.clone(),
                            left,
                        });
                    }
                    if common::icon_button(
                        ui,
                        Icon::Copy,
                        "Copy to other side",
                        common::ButtonIntent::Secondary,
                    )
                    .clicked()
                    {
                        actions.push(UiAction::Copy {
                            rel: data.rel.clone(),
                            from_left: left,
                        });
                    }
                });
            } else {
                ui.label(RichText::new("No file on this side.").color(theme::error()));
            }
        });
}

pub(super) fn error_panel(ui: &mut Ui, data: &OpenData, actions: &mut Vec<UiAction>) {
    ui.label(RichText::new(&data.rel).size(20.0).strong());
    ui.add_space(6.0);
    let message = data
        .error
        .clone()
        .unwrap_or_else(|| "The comparison could not be loaded.".to_string());
    Frame::NONE
        .fill(theme::del_bg())
        .stroke(Stroke::new(1.0_f32, theme::error().gamma_multiply(0.6)))
        .corner_radius(8.0)
        .inner_margin(Margin::symmetric(14, 10))
        .show(ui, |ui| {
            ui.label(RichText::new(message).color(theme::error()));
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                if data.left_path.is_some()
                    && common::icon_button(
                        ui,
                        Icon::External,
                        "Open A",
                        common::ButtonIntent::Ghost,
                    )
                    .clicked()
                {
                    actions.push(UiAction::OpenDefault {
                        rel: data.rel.clone(),
                        left: true,
                    });
                }
                if data.right_path.is_some()
                    && common::icon_button(
                        ui,
                        Icon::External,
                        "Open B",
                        common::ButtonIntent::Ghost,
                    )
                    .clicked()
                {
                    actions.push(UiAction::OpenDefault {
                        rel: data.rel.clone(),
                        left: false,
                    });
                }
            });
        });
    if !data.messages.is_empty() {
        ui.add_space(8.0);
        for message in &data.messages {
            ui.label(RichText::new(message).color(theme::muted()).size(11.0));
        }
    }
}

pub(super) fn error_card(ui: &mut Ui, rel: &str, error: &str, actions: &mut Vec<UiAction>) {
    ui.add_space(24.0);
    Frame::NONE
        .fill(theme::panel())
        .stroke(Stroke::new(1.0_f32, theme::border()))
        .corner_radius(8.0)
        .inner_margin(Margin::symmetric(16, 12))
        .show(ui, |ui| {
            ui.label(RichText::new(rel).size(16.0).strong());
            ui.add_space(4.0);
            ui.label(RichText::new(error).color(theme::error()));
            ui.add_space(8.0);
            if common::icon_button(ui, Icon::Refresh, "Retry", common::ButtonIntent::Secondary)
                .clicked()
            {
                actions.push(UiAction::ReloadOpen);
            }
        });
}

pub(super) fn short_root(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string())
}

pub(super) fn short_path(value: &str) -> String {
    let path = Path::new(value);
    let Some(name) = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
    else {
        return value.to_string();
    };
    let parent = path
        .parent()
        .and_then(Path::file_name)
        .map(|name| name.to_string_lossy().into_owned());
    let label = match parent {
        Some(parent) if !parent.is_empty() => format!("{parent}/{name}"),
        _ => name,
    };
    let chars = label.chars().count();
    if chars > 64 {
        let mut text: String = label.chars().take(60).collect();
        text.push('…');
        text
    } else {
        label
    }
}

#[cfg(target_os = "windows")]
pub(super) fn reveal_in_explorer(path: &Path) {
    use std::os::windows::process::CommandExt;
    let _ = std::process::Command::new("explorer")
        .arg(format!("/select,{}", path.display()))
        .creation_flags(0x08000000)
        .spawn();
}

#[cfg(not(target_os = "windows"))]
pub(super) fn reveal_in_explorer(path: &Path) {
    if let Some(parent) = path.parent() {
        let _ = std::process::Command::new("xdg-open").arg(parent).spawn();
    }
}

#[cfg(target_os = "windows")]
pub(super) fn open_with_default(path: &Path) {
    use std::os::windows::process::CommandExt;
    let _ = std::process::Command::new("cmd")
        .args(["/C", "start", "", &path.display().to_string()])
        .creation_flags(0x08000000)
        .spawn();
}

#[cfg(not(target_os = "windows"))]
pub(super) fn open_with_default(path: &Path) {
    let _ = std::process::Command::new("xdg-open").arg(path).spawn();
}
