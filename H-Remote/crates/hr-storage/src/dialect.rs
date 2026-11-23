//! SQL dialect abstraction.
//!
//! Phase 1 executes SQLite only; the builders are dialect-aware so Phase 8 can
//! add MySQL/PostgreSQL without rewriting the repository call sites.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SqlDialect {
    Sqlite,
    MySql,
    Postgres,
}

impl SqlDialect {
    pub fn placeholder(self, index: usize) -> String {
        match self {
            Self::Postgres => format!("${index}"),
            Self::Sqlite | Self::MySql => "?".to_string(),
        }
    }

    pub fn placeholders(self, count: usize) -> Vec<String> {
        (1..=count).map(|index| self.placeholder(index)).collect()
    }

    pub fn servers_upsert_sql(self) -> String {
        let values = self.placeholders(5).join(", ");
        let sql = format!(
            "INSERT INTO {servers} (Id, Protocol, ClassVersion, Json, UpdatedAt) VALUES ({values})",
            servers = crate::schema::SERVERS
        );
        self.upsert_suffix(
            &sql,
            "Id",
            &["Protocol", "ClassVersion", "Json", "UpdatedAt"],
        )
    }

    pub fn credentials_upsert_sql(self) -> String {
        let values = self.placeholders(5).join(", ");
        let sql = format!(
            "INSERT INTO {credentials} (Id, Name, Hash, Json, UpdatedAt) VALUES ({values})",
            credentials = crate::schema::CREDENTIALS
        );
        self.upsert_suffix(&sql, "Id", &["Name", "Hash", "Json", "UpdatedAt"])
    }

    pub fn configs_upsert_sql(self) -> String {
        let values = self.placeholders(3).join(", ");
        let sql = format!(
            "INSERT INTO {configs} (Key, Value, UpdatedAt) VALUES ({values})",
            configs = crate::schema::CONFIGS
        );
        self.upsert_suffix(&sql, "Key", &["Value", "UpdatedAt"])
    }

    pub fn table_meta_upsert_sql(self) -> String {
        let values = self.placeholders(3).join(", ");
        let sql = format!(
            "INSERT INTO {meta} (TableName, UpdatedAt, RowCount) VALUES ({values})",
            meta = crate::schema::TABLE_META
        );
        self.upsert_suffix(&sql, "TableName", &["UpdatedAt", "RowCount"])
    }

    fn upsert_suffix(self, insert: &str, conflict: &str, updates: &[&str]) -> String {
        match self {
            Self::Sqlite | Self::Postgres => {
                let assignments = updates
                    .iter()
                    .map(|column| format!("{column} = excluded.{column}"))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("{insert} ON CONFLICT({conflict}) DO UPDATE SET {assignments}")
            }
            Self::MySql => {
                let assignments = updates
                    .iter()
                    .map(|column| format!("{column} = VALUES({column})"))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("{insert} ON DUPLICATE KEY UPDATE {assignments}")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn postgres_uses_numbered_placeholders() {
        assert_eq!(SqlDialect::Postgres.placeholder(1), "$1");
        assert_eq!(SqlDialect::Postgres.placeholders(3), vec!["$1", "$2", "$3"]);
        let sql = SqlDialect::Postgres.servers_upsert_sql();
        assert!(sql.contains("VALUES ($1, $2, $3, $4, $5)"));
        assert!(sql.contains("ON CONFLICT(Id)"));
        assert!(sql.contains("Json = excluded.Json"));
    }

    #[test]
    fn sqlite_uses_qmark_and_excluded() {
        let sql = SqlDialect::Sqlite.servers_upsert_sql();
        assert!(sql.contains("VALUES (?, ?, ?, ?, ?)"));
        assert!(sql.contains("ON CONFLICT(Id) DO UPDATE"));
    }

    #[test]
    fn mysql_uses_duplicate_key_update() {
        let sql = SqlDialect::MySql.credentials_upsert_sql();
        assert!(sql.contains("ON DUPLICATE KEY UPDATE"));
        assert!(sql.contains("Name = VALUES(Name)"));
    }
}
