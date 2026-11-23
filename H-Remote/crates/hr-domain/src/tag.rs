//! Tags and tag cascades.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::server::Server;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct Tag {
    pub name: String,
    pub is_pinned: bool,
    pub custom_order: i64,
    pub items_count: usize,
}

impl Tag {
    pub fn new(name: impl Into<String>) -> Option<Self> {
        normalize_tag(&name.into()).map(|name| Self {
            name,
            ..Self::default()
        })
    }
}

/// Normalises `#Prod`, ` Prod ` and `PROD` to `prod`.
pub fn normalize_tag(value: &str) -> Option<String> {
    let trimmed = value.trim().trim_start_matches('#').trim();
    if trimmed.is_empty() || trimmed.chars().any(char::is_whitespace) {
        return None;
    }
    Some(trimmed.to_lowercase())
}

pub fn normalize_tags(values: Vec<String>) -> Vec<String> {
    let mut tags: Vec<String> = values.iter().filter_map(|tag| normalize_tag(tag)).collect();
    tags.sort();
    tags.dedup();
    tags
}

/// Renames a tag on every server. Returns the number of servers changed.
pub fn rename_tag(servers: &mut [Server], old: &str, new: &str) -> usize {
    let (Some(old), Some(new)) = (normalize_tag(old), normalize_tag(new)) else {
        return 0;
    };
    if old == new {
        return 0;
    }
    let mut changed = 0;
    for server in servers {
        let base = server.base_mut();
        let mut updated = false;
        for tag in &mut base.tags {
            if tag == &old {
                *tag = new.clone();
                updated = true;
            }
        }
        if updated {
            base.tags = normalize_tags(std::mem::take(&mut base.tags));
            changed += 1;
        }
    }
    changed
}

/// Removes a tag from every server. Returns the number of servers changed.
pub fn remove_tag(servers: &mut [Server], tag: &str) -> usize {
    let Some(tag) = normalize_tag(tag) else {
        return 0;
    };
    let mut changed = 0;
    for server in servers {
        let base = server.base_mut();
        let before = base.tags.len();
        base.tags.retain(|candidate| candidate != &tag);
        if base.tags.len() != before {
            changed += 1;
        }
    }
    changed
}

/// Collects tags across servers with item counts. Pinned/order flags are
/// merged from the caller's locality state.
pub fn collect_tags(servers: &[Server], pinned: &[String], order: &[String]) -> Vec<Tag> {
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    for server in servers {
        for tag in &server.base().tags {
            *counts.entry(tag.clone()).or_insert(0) += 1;
        }
    }
    let pinned = normalize_tags(pinned.to_vec());
    let order: Vec<(String, i64)> = order
        .iter()
        .enumerate()
        .filter_map(|(index, tag)| {
            normalize_tag(tag).map(|tag| (tag, index.min(i64::MAX as usize) as i64))
        })
        .collect();
    let mut tags: Vec<Tag> = counts
        .into_iter()
        .map(|(name, items_count)| {
            let custom_order = order
                .iter()
                .find(|(candidate, _)| candidate == &name)
                .map(|(_, index)| *index)
                .unwrap_or(i64::MAX);
            Tag {
                is_pinned: pinned.contains(&name),
                custom_order,
                name,
                items_count,
            }
        })
        .collect();
    tags.sort_by(|left, right| {
        left.custom_order
            .cmp(&right.custom_order)
            .then_with(|| left.name.cmp(&right.name))
    });
    tags
}

/// Builds the `Tag` rows shown in the tag panel.
pub fn rethink_tags(servers: &[Server], pinned: &[String], order: &[String]) -> Vec<Tag> {
    collect_tags(servers, pinned, order)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::server::Server;

    #[test]
    fn tags_normalise_reject_spaces_and_deduplicate() {
        assert_eq!(normalize_tag(" #Prod "), Some("prod".to_string()));
        assert_eq!(normalize_tag("my tag"), None);
        assert_eq!(normalize_tag(""), None);
        assert_eq!(
            normalize_tags(vec!["#Prod".into(), "prod".into(), " Lab ".into()]),
            vec!["lab", "prod"]
        );
    }

    #[test]
    fn rename_and_remove_cascade_across_servers() {
        let mut servers = vec![
            Server::new_ssh("a", "one.example", "root").with_tags(["prod", "linux"]),
            Server::new_ssh("b", "two.example", "root").with_tags(["prod"]),
        ];
        assert_eq!(rename_tag(&mut servers, "#prod", "production"), 2);
        assert!(servers[0].base().tags.contains(&"production".to_string()));
        assert_eq!(remove_tag(&mut servers, "production"), 2);
        assert!(
            servers
                .iter()
                .all(|server| !server.base().tags.contains(&"production".to_string()))
        );
        assert_eq!(rename_tag(&mut servers, "missing", "x"), 0);
    }

    #[test]
    fn collected_tags_report_counts_and_order() {
        let servers = vec![
            Server::new_ssh("a", "one", "root").with_tags(["prod", "linux"]),
            Server::new_ssh("b", "two", "root").with_tags(["prod"]),
        ];
        let tags = collect_tags(&servers, &["prod".into()], &["prod".into()]);
        assert_eq!(tags[0].name, "prod");
        assert_eq!(tags[0].items_count, 2);
        assert!(tags[0].is_pinned);
        assert_eq!(tags[1].name, "linux");
        assert_eq!(tags[1].items_count, 1);
        assert!(!tags[1].is_pinned);
    }
}
