//! Domain model for H-Remote.
//!
//! The model is protocol-typed (an enum with one struct per protocol) rather
//! than a bag of optional fields. Common state is shared through flattened
//! bases so JSON stays close to the reference format while Rust code stays
//! exhaustive.

pub mod credential;
pub mod folder;
pub mod icon;
pub mod local_app;
pub mod protocol;
pub mod rdp_file;
pub mod registry;
pub mod search;
pub mod server;
pub mod tag;
pub mod transfer;

pub use credential::{AlternateCredential, Credential, EffectiveCredentials};
pub use folder::{
    Folder, FolderPath, is_descendant_path, migrate_tags_to_tree_nodes, normalize_tree_nodes,
    validate_tree_path,
};
pub use icon::{IconCache, IconData, merge_icons_ico};
pub use local_app::{
    AppArgument, AppArgumentType, ArgumentTemplate, MacroContext, expand_macros, render_arguments,
    templates,
};
pub use protocol::{
    AddressPortBase, GatewayLogonMethod, LOCAL_SOURCE_ID, ProtocolBase, ProtocolKind,
    UserPasswordBase, default_source_id,
};
pub use rdp_file::RdpFile;
pub use registry::*;
pub use search::{
    CatalogFolder, CatalogSort, CatalogTree, MatchProvider, SearchHit, SearchQuery, TextRange,
    build_catalog_tree, highlight_ranges, hit_map, natural_ip_cmp, search, sort_servers,
};
pub use server::{
    AudioQualityMode, AudioRedirectionMode, ControlSettingType, CustomServer, DisplayPerformance,
    DummyServer, FtpServer, GatewayMode, LocalAppServer, ProtocolControlSetting, RdpFullScreenFlag,
    RdpServer, RdpWindowResizeMode, RemoteAppServer, SerialFlowControl, SerialParity, SerialServer,
    Server, SftpServer, SshServer, TelnetServer, ValidationIssue, VncServer, VncWindowResizeMode,
    sample_servers,
};
pub use tag::{Tag, collect_tags, normalize_tag, normalize_tags, remove_tag, rename_tag};
pub use transfer::{
    EXPORT_SCHEMA_VERSION, ExportEnvelope, ImportResult, export_json, export_rdp, import_json,
    import_mremoteng_csv, import_rdp, legacy_value_to_server,
};
