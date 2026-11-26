//! SQL schema constants and migrations.

pub const SERVERS: &str = "Servers";
pub const CREDENTIALS: &str = "Credentials";
pub const CONFIGS: &str = "Configs";
pub const TABLE_META: &str = "TableMeta";
pub const FOLDERS: &str = "Folders";
pub const ENCRYPTION_TEST_KEY: &str = "EncryptionTest";

pub const CURRENT_SCHEMA_VERSION: i64 = 4;

pub const MIGRATIONS: &[(i64, &str)] = &[
    (
        1,
        "CREATE TABLE IF NOT EXISTS _hremote_meta (
            key TEXT PRIMARY KEY NOT NULL,
            value TEXT NOT NULL,
            updated_at TEXT NOT NULL DEFAULT (datetime('now'))
        );",
    ),
    (
        2,
        "CREATE TABLE IF NOT EXISTS Servers (
            Id TEXT PRIMARY KEY NOT NULL,
            Protocol TEXT NOT NULL,
            ClassVersion INTEGER NOT NULL,
            Json TEXT NOT NULL,
            UpdatedAt INTEGER NOT NULL
        );
        CREATE INDEX IF NOT EXISTS Servers_Protocol ON Servers(Protocol);
        CREATE INDEX IF NOT EXISTS Servers_UpdatedAt ON Servers(UpdatedAt);

        CREATE TABLE IF NOT EXISTS Credentials (
            Id TEXT PRIMARY KEY NOT NULL,
            Name TEXT NOT NULL,
            Hash TEXT NOT NULL,
            Json TEXT NOT NULL,
            UpdatedAt INTEGER NOT NULL
        );
        CREATE UNIQUE INDEX IF NOT EXISTS Credentials_Name ON Credentials(Name COLLATE NOCASE);
        CREATE INDEX IF NOT EXISTS Credentials_Hash ON Credentials(Hash);

        CREATE TABLE IF NOT EXISTS Configs (
            Key TEXT PRIMARY KEY NOT NULL,
            Value TEXT NOT NULL,
            UpdatedAt INTEGER NOT NULL
        );

        CREATE TABLE IF NOT EXISTS TableMeta (
            TableName TEXT PRIMARY KEY NOT NULL,
            UpdatedAt INTEGER NOT NULL,
            RowCount INTEGER NOT NULL DEFAULT 0
        );
        INSERT INTO TableMeta (TableName, UpdatedAt, RowCount) VALUES ('Servers', 0, 0)
            ON CONFLICT(TableName) DO NOTHING;
        INSERT INTO TableMeta (TableName, UpdatedAt, RowCount) VALUES ('Credentials', 0, 0)
            ON CONFLICT(TableName) DO NOTHING;",
    ),
    (
        3,
        "CREATE TABLE IF NOT EXISTS Folders (
            Id TEXT PRIMARY KEY NOT NULL,
            Path TEXT NOT NULL,
            UpdatedAt INTEGER NOT NULL
        );
        CREATE UNIQUE INDEX IF NOT EXISTS Folders_Path ON Folders(Path);
        INSERT INTO TableMeta (TableName, UpdatedAt, RowCount) VALUES ('Folders', 0, 0)
            ON CONFLICT(TableName) DO NOTHING;",
    ),
    (
        4,
        "ALTER TABLE Folders ADD COLUMN ColorHex TEXT NOT NULL DEFAULT '';",
    ),
];
