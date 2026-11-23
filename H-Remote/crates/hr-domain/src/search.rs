//! Catalog search, matching providers, natural sorting and folder grouping.

use std::cmp::Ordering;
use std::collections::{BTreeMap, HashMap};

use pinyin::ToPinyin;
use serde::{Deserialize, Serialize};

use crate::folder::Folder;
use crate::server::Server;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum MatchProvider {
    #[default]
    Direct,
    Initials,
    Discrete,
    Pinyin,
    PinyinInitials,
}

impl MatchProvider {
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "direct" => Some(Self::Direct),
            "initials" => Some(Self::Initials),
            "discrete" => Some(Self::Discrete),
            "pinyin" => Some(Self::Pinyin),
            "pinyininitials" | "pinyin_initials" | "pinyin-initials" => Some(Self::PinyinInitials),
            _ => None,
        }
    }

    /// Direct matching is mandatory whenever providers are configured.
    pub fn normalize(providers: impl IntoIterator<Item = Self>) -> Vec<Self> {
        let mut result = vec![Self::Direct];
        for provider in providers {
            if !result.contains(&provider) {
                result.push(provider);
            }
        }
        result
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchQuery {
    pub keywords: Vec<String>,
    pub include_tags: Vec<String>,
    pub exclude_tags: Vec<String>,
}

impl SearchQuery {
    pub fn parse(input: &str) -> Self {
        let mut query = Self::default();
        for token in input.split_whitespace() {
            let token = token.trim();
            if token.is_empty() {
                continue;
            }
            if let Some(tag) = token.strip_prefix("-#") {
                if let Some(tag) = normalize_tag_prefix(tag) {
                    query.exclude_tags.push(tag);
                }
            } else if let Some(tag) = token.strip_prefix("+#") {
                if let Some(tag) = normalize_tag_prefix(tag) {
                    query.include_tags.push(tag);
                }
            } else if let Some(tag) = token.strip_prefix('#') {
                if let Some(tag) = normalize_tag_prefix(tag) {
                    query.include_tags.push(tag);
                }
            } else {
                query.keywords.push(token.to_lowercase());
            }
        }
        query.include_tags.sort();
        query.include_tags.dedup();
        query.exclude_tags.sort();
        query.exclude_tags.dedup();
        query
    }

    pub fn is_empty(&self) -> bool {
        self.keywords.is_empty() && self.include_tags.is_empty() && self.exclude_tags.is_empty()
    }

    pub fn tags_match(&self, server: &Server) -> bool {
        let tags = &server.base().tags;
        let includes = self.include_tags.iter().all(|wanted| {
            tags.iter()
                .any(|tag| tag.to_lowercase().starts_with(wanted))
        });
        let excludes = self.exclude_tags.iter().all(|unwanted| {
            !tags
                .iter()
                .any(|tag| tag.to_lowercase().starts_with(unwanted))
        });
        includes && excludes
    }
}

fn normalize_tag_prefix(value: &str) -> Option<String> {
    let tag = value.trim().trim_start_matches('#').to_lowercase();
    if tag.is_empty() || tag.chars().any(char::is_whitespace) {
        None
    } else {
        Some(tag)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextRange {
    /// Character (not byte) indices, suitable for JavaScript slicing.
    pub start: usize,
    pub end: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchHit {
    pub id: String,
    pub name_ranges: Vec<TextRange>,
    pub subtitle_ranges: Vec<TextRange>,
}

pub fn search(
    servers: &[Server],
    query: &SearchQuery,
    providers: &[MatchProvider],
) -> Vec<SearchHit> {
    if query.is_empty() {
        return servers
            .iter()
            .map(|server| SearchHit {
                id: server.base().id.clone(),
                name_ranges: Vec::new(),
                subtitle_ranges: Vec::new(),
            })
            .collect();
    }
    let providers = MatchProvider::normalize(providers.iter().copied());
    servers
        .iter()
        .filter(|server| query.tags_match(server))
        .filter(|server| keywords_match(server, &query.keywords, &providers))
        .map(|server| SearchHit {
            id: server.base().id.clone(),
            name_ranges: highlight_ranges(&server.base().display_name, &query.keywords),
            subtitle_ranges: highlight_ranges(&server.base().sub_title, &query.keywords),
        })
        .collect()
}

fn keywords_match(server: &Server, keywords: &[String], providers: &[MatchProvider]) -> bool {
    if keywords.is_empty() {
        return true;
    }
    keywords.iter().all(|keyword| {
        providers
            .iter()
            .any(|provider| provider_matches(server, keyword, *provider))
    })
}

fn provider_matches(server: &Server, keyword: &str, provider: MatchProvider) -> bool {
    let name = server.base().display_name.to_lowercase();
    let subtitle = server.base().sub_title.to_lowercase();
    match provider {
        MatchProvider::Direct => name.contains(keyword) || subtitle.contains(keyword),
        MatchProvider::Initials => initials(&name).contains(keyword),
        MatchProvider::Discrete => is_subsequence(keyword, &name),
        MatchProvider::Pinyin => pinyin_plain(&name).contains(keyword),
        MatchProvider::PinyinInitials => pinyin_initials(&name).contains(keyword),
    }
}

fn initials(value: &str) -> String {
    value
        .split(|character: char| !character.is_alphanumeric())
        .filter(|part| !part.is_empty())
        .filter_map(|part| part.chars().next())
        .collect::<String>()
        .to_lowercase()
}

fn is_subsequence(needle: &str, haystack: &str) -> bool {
    let mut chars = haystack.chars();
    needle
        .chars()
        .all(|wanted| chars.any(|candidate| candidate == wanted))
}

fn pinyin_plain(value: &str) -> String {
    value
        .to_pinyin()
        .flatten()
        .map(|pinyin| pinyin.plain())
        .collect::<String>()
        .to_lowercase()
}

fn pinyin_initials(value: &str) -> String {
    value
        .to_pinyin()
        .flatten()
        .map(|pinyin| pinyin.first_letter())
        .collect::<String>()
        .to_lowercase()
}

/// Finds every direct case-insensitive occurrence and merges overlaps.
pub fn highlight_ranges(text: &str, keywords: &[String]) -> Vec<TextRange> {
    if keywords.is_empty() {
        return Vec::new();
    }
    let lower: Vec<char> = text.to_lowercase().chars().collect();
    let mut ranges = Vec::new();
    for keyword in keywords {
        let needle: Vec<char> = keyword.chars().collect();
        if needle.is_empty() || needle.len() > lower.len() {
            continue;
        }
        for start in 0..=lower.len() - needle.len() {
            if lower[start..start + needle.len()] == needle {
                ranges.push(TextRange {
                    start,
                    end: start + needle.len(),
                });
            }
        }
    }
    ranges.sort_by_key(|range| (range.start, range.end));
    let mut merged: Vec<TextRange> = Vec::new();
    for range in ranges {
        if let Some(last) = merged.last_mut()
            && range.start <= last.end
        {
            last.end = last.end.max(range.end);
            continue;
        }
        merged.push(range);
    }
    merged
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum CatalogSort {
    #[default]
    Insertion,
    Protocol,
    Name,
    Address,
    Custom,
}

pub fn sort_servers(servers: &mut [Server], sort: CatalogSort, custom_order: &[String]) {
    servers.sort_by(|left, right| compare_servers(left, right, sort, custom_order));
}

fn compare_servers(
    left: &Server,
    right: &Server,
    sort: CatalogSort,
    custom_order: &[String],
) -> Ordering {
    let ordering = match sort {
        CatalogSort::Insertion => left.base().id.cmp(&right.base().id),
        CatalogSort::Protocol => left
            .protocol()
            .id()
            .cmp(right.protocol().id())
            .then_with(|| {
                left.base()
                    .display_name
                    .to_lowercase()
                    .cmp(&right.base().display_name.to_lowercase())
            }),
        CatalogSort::Name => left
            .base()
            .display_name
            .to_lowercase()
            .cmp(&right.base().display_name.to_lowercase()),
        CatalogSort::Address => natural_ip_cmp(
            left.address()
                .map(|address| address.address.as_str())
                .unwrap_or(""),
            right
                .address()
                .map(|address| address.address.as_str())
                .unwrap_or(""),
        ),
        CatalogSort::Custom => {
            let left_index = custom_order
                .iter()
                .position(|id| id == &left.base().id)
                .unwrap_or(usize::MAX);
            let right_index = custom_order
                .iter()
                .position(|id| id == &right.base().id)
                .unwrap_or(usize::MAX);
            left_index.cmp(&right_index)
        }
    };
    ordering.then_with(|| left.base().id.cmp(&right.base().id))
}

/// Natural comparison so `10.0.0.10` sorts after `10.0.0.2`.
pub fn natural_ip_cmp(left: &str, right: &str) -> Ordering {
    let left_parts = ip_parts(left);
    let right_parts = ip_parts(right);
    for (left, right) in left_parts.iter().zip(&right_parts) {
        let ordering = match (left.parse::<u128>(), right.parse::<u128>()) {
            (Ok(left), Ok(right)) => left.cmp(&right),
            _ => left.to_lowercase().cmp(&right.to_lowercase()),
        };
        if ordering != Ordering::Equal {
            return ordering;
        }
    }
    left_parts.len().cmp(&right_parts.len())
}

fn ip_parts(value: &str) -> Vec<String> {
    value
        .split(|character: char| !character.is_ascii_alphanumeric())
        .filter(|part| !part.is_empty())
        .map(str::to_string)
        .collect()
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogFolder {
    pub path: Vec<String>,
    pub name: String,
    pub folders: Vec<CatalogFolder>,
    pub servers: Vec<Server>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogTree {
    pub folders: Vec<CatalogFolder>,
    pub servers: Vec<Server>,
}

/// Builds the nested folder tree used by List/Card and Tree views.
/// Folders always sort before directly-contained servers (P2-17b).
pub fn build_catalog_tree(
    servers: &[Server],
    folders: &[Folder],
    sort: CatalogSort,
    custom_order: &[String],
    folder_order: &[String],
) -> CatalogTree {
    #[derive(Default)]
    struct Node {
        path: Vec<String>,
        children: BTreeMap<String, Node>,
        servers: Vec<Server>,
    }

    let mut root = Node::default();
    let ensure = |root: &mut Node, path: &[String]| {
        let mut current = root;
        let mut prefix = Vec::new();
        for segment in path {
            prefix.push(segment.clone());
            current = current
                .children
                .entry(segment.clone())
                .or_insert_with(|| Node {
                    path: prefix.clone(),
                    ..Node::default()
                });
        }
    };

    for folder in folders {
        ensure(&mut root, &folder.path);
    }
    for server in servers {
        ensure(&mut root, &server.base().tree_nodes);
        let mut current = &mut root;
        for segment in &server.base().tree_nodes {
            current = current
                .children
                .get_mut(segment)
                .expect("folder path was just created");
        }
        current.servers.push(server.clone());
    }

    fn convert(
        node: Node,
        sort: CatalogSort,
        custom_order: &[String],
        folder_order: &[String],
    ) -> Vec<CatalogFolder> {
        let mut children: Vec<CatalogFolder> = node
            .children
            .into_values()
            .flat_map(|child| convert(child, sort, custom_order, folder_order))
            .collect();
        children.sort_by(|left, right| {
            let left_key = left.path.join("/");
            let right_key = right.path.join("/");
            let left_index = folder_order
                .iter()
                .position(|path| path == &left_key)
                .unwrap_or(usize::MAX);
            let right_index = folder_order
                .iter()
                .position(|path| path == &right_key)
                .unwrap_or(usize::MAX);
            left_index
                .cmp(&right_index)
                .then_with(|| left.name.to_lowercase().cmp(&right.name.to_lowercase()))
        });
        let mut servers = node.servers;
        sort_servers(&mut servers, sort, custom_order);
        let this = CatalogFolder {
            name: node.path.last().cloned().unwrap_or_default(),
            path: node.path,
            folders: children,
            servers,
        };
        vec![this]
    }

    let mut folders_out = Vec::new();
    for child in root.children.into_values() {
        folders_out.extend(convert(child, sort, custom_order, folder_order));
    }
    // `convert` returns each node as a one-element vector; flatten top level.
    let mut root_servers = root.servers;
    sort_servers(&mut root_servers, sort, custom_order);
    CatalogTree {
        folders: folders_out,
        servers: root_servers,
    }
}

/// Flat map of server id -> search hit, used by the UI to highlight rows.
pub fn hit_map(hits: &[SearchHit]) -> HashMap<&str, &SearchHit> {
    hits.iter().map(|hit| (hit.id.as_str(), hit)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn server(name: &str, address: &str, tags: &[&str]) -> Server {
        Server::new_ssh(name, address, "root").with_tags(tags.iter().copied())
    }

    #[test]
    fn query_parses_tag_operators_and_partial_tags() {
        let query = SearchQuery::parse("web #prod -#win +#lin #de");
        assert_eq!(query.keywords, vec!["web"]);
        assert_eq!(query.include_tags, vec!["de", "lin", "prod"]);
        assert_eq!(query.exclude_tags, vec!["win"]);
    }

    #[test]
    fn tag_filters_are_prefix_based_and_exclusive() {
        let server = server("Web", "host", &["production", "linux"]);
        assert!(SearchQuery::parse("#prod #lin").tags_match(&server));
        assert!(!SearchQuery::parse("#prod -#lin").tags_match(&server));
        assert!(!SearchQuery::parse("#win").tags_match(&server));
        assert!(SearchQuery::parse("").tags_match(&server));
    }

    #[test]
    fn direct_search_matches_name_and_subtitle_with_highlights() {
        let servers = vec![
            server("Prod Web", "10.0.0.1", &[]),
            server("Database", "db.internal", &[]),
        ];
        let hits = search(
            &servers,
            &SearchQuery::parse("web"),
            &[MatchProvider::Direct],
        );
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].name_ranges, vec![TextRange { start: 5, end: 8 }]);

        let hits = search(
            &servers,
            &SearchQuery::parse("db.internal"),
            &[MatchProvider::Direct],
        );
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].subtitle_ranges[0].start, 0);
    }

    #[test]
    fn initials_discrete_and_pinyin_providers_work() {
        let servers = vec![
            server("Production Web", "host", &[]),
            server("开发服务器", "cn", &[]),
        ];
        assert_eq!(
            search(
                &servers,
                &SearchQuery::parse("pw"),
                &[MatchProvider::Initials]
            )
            .len(),
            1
        );
        assert_eq!(
            search(
                &servers,
                &SearchQuery::parse("pdn"),
                &[MatchProvider::Discrete]
            )
            .len(),
            1
        );
        assert_eq!(
            search(
                &servers,
                &SearchQuery::parse("kaifa"),
                &[MatchProvider::Pinyin]
            )
            .len(),
            1
        );
        assert_eq!(
            search(
                &servers,
                &SearchQuery::parse("kffwq"),
                &[MatchProvider::PinyinInitials]
            )
            .len(),
            1
        );
    }

    #[test]
    fn direct_provider_is_always_kept() {
        assert_eq!(
            MatchProvider::normalize([MatchProvider::Pinyin, MatchProvider::Direct]),
            vec![MatchProvider::Direct, MatchProvider::Pinyin]
        );
    }

    #[test]
    fn natural_ip_sort_orders_numeric_segments() {
        let mut values = vec!["10.0.0.10", "10.0.0.2", "host.example", "10.0.0.1"];
        values.sort_by(|left, right| natural_ip_cmp(left, right));
        assert_eq!(
            values,
            vec!["10.0.0.1", "10.0.0.2", "10.0.0.10", "host.example"]
        );
    }

    #[test]
    fn folder_tree_is_nested_and_folders_come_first() {
        let mut prod = server("Prod", "10.0.0.2", &[]);
        prod.base_mut().tree_nodes = vec!["Prod".into()];
        let mut child = server("Child", "10.0.0.1", &[]);
        child.base_mut().tree_nodes = vec!["Prod".into(), "EU".into()];
        let root = server("Root", "10.0.0.3", &[]);
        let folders = vec![
            Folder::new("1", vec!["Prod".into()]),
            Folder::new("2", vec!["Prod".into(), "EU".into()]),
            Folder::new("3", vec!["Empty".into()]),
        ];
        let tree = build_catalog_tree(
            &[prod, child, root.clone()],
            &folders,
            CatalogSort::Address,
            &[],
            &[],
        );
        assert_eq!(tree.folders.len(), 2);
        assert_eq!(tree.servers, vec![root]);
        let prod_folder = tree
            .folders
            .iter()
            .find(|folder| folder.name == "Prod")
            .unwrap();
        assert_eq!(prod_folder.servers[0].base().display_name, "Prod");
        assert_eq!(prod_folder.folders[0].name, "EU");
        assert!(
            prod_folder.folders[0].servers[0]
                .base()
                .sub_title
                .contains("10.0.0.1")
        );
    }

    #[test]
    fn custom_sort_uses_persisted_order() {
        let mut servers = vec![
            server("B", "b", &[]),
            server("A", "a", &[]),
            server("C", "c", &[]),
        ];
        let order = vec![
            servers[1].base().id.clone(),
            servers[0].base().id.clone(),
            servers[2].base().id.clone(),
        ];
        sort_servers(&mut servers, CatalogSort::Custom, &order);
        assert_eq!(
            servers
                .iter()
                .map(|s| s.base().display_name.as_str())
                .collect::<Vec<_>>(),
            vec!["A", "B", "C"]
        );
    }

    #[test]
    fn highlight_ranges_merge_overlaps() {
        assert_eq!(
            highlight_ranges("aaaa", &["aa".into()]),
            vec![TextRange { start: 0, end: 4 }]
        );
        assert_eq!(
            highlight_ranges("abc", &["abc".into()]),
            vec![TextRange { start: 0, end: 3 }]
        );
    }

    #[test]
    fn hit_map_indexes_by_server_id() {
        let hits = search(
            &[server("A", "a", &[])],
            &SearchQuery::parse("a"),
            &[MatchProvider::Direct],
        );
        let map = hit_map(&hits);
        assert!(map.contains_key(hits[0].id.as_str()));
    }
}
