use hr_config::WindowPlacement;
use serde::{Deserialize, Serialize};

const MIN_WIDTH: u32 = 480;
const MIN_HEIGHT: u32 = 320;
const DEFAULT_WIDTH: u32 = 1200;
const DEFAULT_HEIGHT: u32 = 800;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MonitorInfo {
    pub name: Option<String>,
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub scale_factor: f64,
    pub is_primary: bool,
}

impl MonitorInfo {
    pub fn contains(&self, x: i32, y: i32) -> bool {
        x >= self.x
            && y >= self.y
            && x < self.x.saturating_add(self.width as i32)
            && y < self.y.saturating_add(self.height as i32)
    }

    pub fn center(&self) -> (i32, i32) {
        (
            self.x.saturating_add(self.width as i32 / 2),
            self.y.saturating_add(self.height as i32 / 2),
        )
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlacementDecision {
    pub placement: WindowPlacement,
    pub adjusted: bool,
    pub reason: Option<String>,
}

/// Validates a stored placement against the current monitor layout (P0-42/43).
/// The returned placement always fits inside a real monitor, so a laptop
/// undocked from an external display never opens a window off-screen.
pub fn validate_placement(
    placement: &WindowPlacement,
    monitors: &[MonitorInfo],
) -> PlacementDecision {
    let mut result = placement.clone();
    let signature = screen_layout_signature(monitors);

    if monitors.is_empty() {
        result.width = result.width.max(MIN_WIDTH);
        result.height = result.height.max(MIN_HEIGHT);
        result.layout_signature = Some(signature);
        let adjusted = result != *placement;
        return PlacementDecision {
            placement: result,
            adjusted,
            reason: Some("no monitors detected; using stored geometry".to_string()),
        };
    }

    let target = placement
        .monitor_name
        .as_ref()
        .and_then(|name| {
            monitors.iter().find(|monitor| {
                monitor
                    .name
                    .as_deref()
                    .is_some_and(|candidate| candidate == name)
            })
        })
        .or_else(|| {
            let center_x = placement.x.saturating_add(placement.width as i32 / 2);
            let center_y = placement.y.saturating_add(placement.height as i32 / 2);
            monitors
                .iter()
                .find(|monitor| monitor.contains(center_x, center_y))
        })
        .or_else(|| monitors.iter().find(|monitor| monitor.is_primary))
        .unwrap_or(&monitors[0]);

    let mut adjusted = false;
    let max_width = target.width.max(MIN_WIDTH);
    let max_height = target.height.max(MIN_HEIGHT);
    let desired_width = if placement.width == 0 {
        DEFAULT_WIDTH
    } else {
        placement.width
    };
    let desired_height = if placement.height == 0 {
        DEFAULT_HEIGHT
    } else {
        placement.height
    };
    result.width = desired_width.clamp(MIN_WIDTH, max_width);
    result.height = desired_height.clamp(MIN_HEIGHT, max_height);
    adjusted |= result.width != placement.width || result.height != placement.height;

    let center_x = placement.x.saturating_add(placement.width as i32 / 2);
    let center_y = placement.y.saturating_add(placement.height as i32 / 2);
    let on_target = target.contains(center_x, center_y);
    if !on_target {
        result.x = target
            .x
            .saturating_add((target.width.saturating_sub(result.width) / 2) as i32);
        result.y = target
            .y
            .saturating_add((target.height.saturating_sub(result.height) / 2) as i32);
        adjusted = true;
    }

    let max_x = target
        .x
        .saturating_add(target.width.saturating_sub(result.width) as i32);
    let max_y = target
        .y
        .saturating_add(target.height.saturating_sub(result.height) as i32);
    let clamped_x = result.x.clamp(target.x, max_x);
    let clamped_y = result.y.clamp(target.y, max_y);
    adjusted |= clamped_x != result.x || clamped_y != result.y;
    result.x = clamped_x;
    result.y = clamped_y;

    let monitor_name = target.name.clone();
    let scale = Some(target.scale_factor);
    adjusted |= result.monitor_name != monitor_name;
    adjusted |= result.scale_factor != scale;
    adjusted |= result.layout_signature.as_deref() != Some(signature.as_str());
    result.monitor_name = monitor_name;
    result.scale_factor = scale;
    result.layout_signature = Some(signature);

    PlacementDecision {
        placement: result,
        adjusted,
        reason: if adjusted {
            Some("window placement was adjusted to the current monitor layout".to_string())
        } else {
            None
        },
    }
}

/// Stable identifier of the monitor layout. Used to decide whether "prefer
/// last opened display" still makes sense after a dock/undock.
pub fn screen_layout_signature(monitors: &[MonitorInfo]) -> String {
    let mut ordered: Vec<&MonitorInfo> = monitors.iter().collect();
    ordered.sort_by(|left, right| {
        left.x
            .cmp(&right.x)
            .then_with(|| left.y.cmp(&right.y))
            .then_with(|| left.name.cmp(&right.name))
    });
    ordered
        .into_iter()
        .map(|monitor| {
            format!(
                "{}@{},{}:{}x{}@{:.2}",
                monitor.name.as_deref().unwrap_or("unnamed"),
                monitor.x,
                monitor.y,
                monitor.width,
                monitor.height,
                monitor.scale_factor
            )
        })
        .collect::<Vec<_>>()
        .join("|")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn primary() -> MonitorInfo {
        MonitorInfo {
            name: Some("Primary".into()),
            x: 0,
            y: 0,
            width: 1920,
            height: 1080,
            scale_factor: 1.0,
            is_primary: true,
        }
    }

    fn secondary() -> MonitorInfo {
        MonitorInfo {
            name: Some("Secondary".into()),
            x: 1920,
            y: 0,
            width: 2560,
            height: 1440,
            scale_factor: 1.5,
            is_primary: false,
        }
    }

    fn placement(x: i32, y: i32, width: u32, height: u32) -> WindowPlacement {
        WindowPlacement {
            x,
            y,
            width,
            height,
            ..WindowPlacement::default()
        }
    }

    #[test]
    fn on_screen_placement_is_kept() {
        let monitors = vec![primary(), secondary()];
        let stored = placement(200, 120, 1000, 700);
        let decision = validate_placement(&stored, &monitors);
        assert_eq!(decision.placement.x, 200);
        assert_eq!(decision.placement.y, 120);
        assert_eq!(decision.placement.monitor_name.as_deref(), Some("Primary"));
        assert_eq!(decision.placement.scale_factor, Some(1.0));
        assert!(decision.placement.layout_signature.is_some());
    }

    #[test]
    fn off_screen_placement_is_recentered() {
        let monitors = vec![primary()];
        let stored = placement(20_000, 20_000, 1000, 700);
        let decision = validate_placement(&stored, &monitors);
        assert!(decision.adjusted);
        assert!(decision.placement.x >= 0);
        assert!(decision.placement.y >= 0);
        assert!(decision.placement.x + decision.placement.width as i32 <= 1920);
        assert!(decision.placement.y + decision.placement.height as i32 <= 1080);
    }

    #[test]
    fn oversized_windows_are_clamped_to_the_monitor() {
        let monitors = vec![primary()];
        let stored = placement(0, 0, 5000, 4000);
        let decision = validate_placement(&stored, &monitors);
        assert_eq!(decision.placement.width, 1920);
        assert_eq!(decision.placement.height, 1080);
        assert!(decision.adjusted);
    }

    #[test]
    fn saved_monitor_name_wins() {
        let monitors = vec![primary(), secondary()];
        let mut stored = placement(100, 100, 900, 600);
        stored.monitor_name = Some("Secondary".into());
        let decision = validate_placement(&stored, &monitors);
        assert_eq!(decision.placement.y, (1440 - 600) / 2);
        assert_eq!(decision.placement.scale_factor, Some(1.5));
        assert!(decision.placement.x >= 1920);
    }

    #[test]
    fn missing_monitor_falls_back_to_primary() {
        let monitors = vec![secondary(), primary()];
        let mut stored = placement(100, 100, 800, 600);
        stored.monitor_name = Some("Removed".into());
        let decision = validate_placement(&stored, &monitors);
        assert_eq!(decision.placement.monitor_name.as_deref(), Some("Primary"));
    }

    #[test]
    fn no_monitors_still_returns_safe_geometry() {
        let stored = placement(0, 0, 0, 0);
        let decision = validate_placement(&stored, &[]);
        assert_eq!(decision.placement.width, MIN_WIDTH);
        assert_eq!(decision.placement.height, MIN_HEIGHT);
        assert!(decision.reason.is_some());
    }

    #[test]
    fn layout_signature_is_stable_regardless_of_input_order() {
        let first = screen_layout_signature(&[primary(), secondary()]);
        let second = screen_layout_signature(&[secondary(), primary()]);
        assert_eq!(first, second);
        assert!(first.contains("Primary@0,0:1920x1080@1.00"));
        assert!(first.contains("Secondary@1920,0:2560x1440@1.50"));
    }
}
