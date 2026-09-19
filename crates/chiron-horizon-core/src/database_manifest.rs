use std::sync::OnceLock;

use serde::Deserialize;

use crate::models::connection::DatabaseType;

const DATABASE_MANIFEST_JSON: &str = include_str!(concat!(env!("OUT_DIR"), "/database_manifest.json"));

#[derive(Debug, Deserialize)]
struct DatabaseManifestFile {
    drivers: Vec<DatabaseManifestEntry>,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum DatabaseRuntimeMode {
    Native,
    File,
    Agent,
    External,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DatabaseAccessContract {
    pub connection_picker: String,
    pub runtime_provider: String,
    pub connect_supported: bool,
    pub test_connection_supported: bool,
    #[serde(default)]
    pub required_features: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DatabaseSchemaViewerContract {
    pub kind: String,
    pub provider: String,
    pub scope_levels: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DatabaseDriverProfile {
    pub profile: String,
    pub agent_key: String,
    #[serde(default)]
    pub package_key: Option<String>,
    pub label: String,
    #[serde(default)]
    pub store_visible: bool,
    #[serde(default)]
    pub store_order: Option<u32>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManagedDriverEntry {
    pub key: String,
    pub label: String,
    #[serde(default)]
    pub store_visible: bool,
    #[serde(default)]
    pub store_order: Option<u32>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DatabaseManifestEntry {
    pub db_type: DatabaseType,
    pub label: String,
    #[serde(default)]
    pub dialect: Option<String>,
    pub runtime_mode: DatabaseRuntimeMode,
    pub mcp_mode: String,
    #[serde(default)]
    pub agent_key: Option<String>,
    #[serde(default)]
    pub driver_store_visible: bool,
    #[serde(default)]
    pub driver_store_order: Option<u32>,
    #[serde(default)]
    pub driver_profiles: Vec<DatabaseDriverProfile>,
    #[serde(default)]
    pub managed_drivers: Vec<ManagedDriverEntry>,
    #[serde(default)]
    pub single_connection_pool: bool,
    #[serde(default)]
    pub metadata_connection_scoped: bool,
    #[serde(default)]
    pub skip_tcp_probe: bool,
    #[serde(default)]
    pub default_port: Option<u16>,
    #[serde(default)]
    pub local_file: bool,
    #[serde(default)]
    pub specialized_surface: bool,
    pub access: DatabaseAccessContract,
    pub schema_viewer: DatabaseSchemaViewerContract,
}

static DATABASE_MANIFEST: OnceLock<DatabaseManifestFile> = OnceLock::new();

fn manifest() -> &'static DatabaseManifestFile {
    DATABASE_MANIFEST.get_or_init(|| {
        serde_json::from_str(DATABASE_MANIFEST_JSON).expect("database-drivers.manifest.json must be valid")
    })
}

pub fn entries() -> &'static [DatabaseManifestEntry] {
    &manifest().drivers
}

pub fn entry(db_type: &DatabaseType) -> Option<&'static DatabaseManifestEntry> {
    entries().iter().find(|candidate| candidate.db_type == *db_type)
}

pub fn default_port(db_type: &DatabaseType) -> Option<u16> {
    entry(db_type).and_then(|candidate| candidate.default_port.or(candidate.local_file.then_some(0)))
}

pub fn agent_key(db_type: &DatabaseType) -> Option<&'static str> {
    entry(db_type).and_then(|candidate| candidate.agent_key.as_deref())
}

pub fn dialect_name(db_type: &DatabaseType) -> Option<&'static str> {
    entry(db_type).and_then(|candidate| candidate.dialect.as_deref())
}

pub fn is_agent_runtime(db_type: &DatabaseType) -> bool {
    entry(db_type).is_some_and(|candidate| candidate.runtime_mode == DatabaseRuntimeMode::Agent)
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    #[test]
    fn manifest_entries_have_unique_database_types() {
        let mut seen = HashSet::new();
        assert!(entries().iter().all(|entry| seen.insert(entry.db_type)));
        assert_eq!(seen.len(), DatabaseType::ALL.len());
        assert!(DatabaseType::ALL.iter().all(|db_type| entry(db_type).is_some()));
    }

    #[test]
    fn database_type_strings_match_serde_contract() {
        for db_type in DatabaseType::ALL {
            let serialized = serde_json::to_string(db_type).expect("database type must serialize");
            assert_eq!(serialized, format!("\"{}\"", db_type.as_str()));
            let deserialized =
                serde_json::from_str::<DatabaseType>(&serialized).expect("database type must deserialize");
            assert_eq!(deserialized, *db_type);
        }
    }

    #[test]
    fn manifest_contains_connection_runtime_defaults() {
        assert_eq!(entry(&DatabaseType::Mysql).map(|entry| entry.default_port), Some(Some(3306)));
        assert_eq!(entry(&DatabaseType::Postgres).map(|entry| entry.default_port), Some(Some(5432)));
        assert_eq!(entry(&DatabaseType::Sqlite).map(|entry| entry.skip_tcp_probe), Some(true));
        assert_eq!(entry(&DatabaseType::H2).map(|entry| entry.agent_key.as_deref()), Some(Some("h2")));
        assert!(entries().iter().all(|entry| entry.access.connect_supported
            && entry.access.test_connection_supported
            && !entry.access.connection_picker.is_empty()
            && !entry.access.runtime_provider.is_empty()));
        assert!(entries().iter().all(|entry| !entry.schema_viewer.kind.is_empty()
            && !entry.schema_viewer.provider.is_empty()
            && !entry.schema_viewer.scope_levels.is_empty()));
    }

    #[test]
    fn access_contracts_match_real_runtime_registration() {
        for entry in entries() {
            let provider = entry.access.runtime_provider.as_str();
            match provider {
                "native" => assert!(
                    matches!(entry.runtime_mode, DatabaseRuntimeMode::Native | DatabaseRuntimeMode::File),
                    "{} declares native access with {:?} runtime",
                    entry.db_type.as_str(),
                    entry.runtime_mode,
                ),
                "agent" => {
                    assert_eq!(entry.runtime_mode, DatabaseRuntimeMode::Agent, "{}", entry.db_type.as_str());
                    assert!(
                        entry.agent_key.as_deref().is_some_and(|key| !key.is_empty()),
                        "{}",
                        entry.db_type.as_str()
                    );
                }
                "jdbc" => assert_eq!(entry.runtime_mode, DatabaseRuntimeMode::External, "{}", entry.db_type.as_str()),
                "external-plugin" => {
                    assert_eq!(entry.runtime_mode, DatabaseRuntimeMode::External, "{}", entry.db_type.as_str());
                    assert_eq!(entry.db_type, DatabaseType::Plugin);
                }
                "specialized-service" => assert!(
                    matches!(entry.runtime_mode, DatabaseRuntimeMode::Native | DatabaseRuntimeMode::Agent),
                    "{}",
                    entry.db_type.as_str(),
                ),
                other => panic!("{} has unknown runtime provider {other}", entry.db_type.as_str()),
            }
        }
    }
}
