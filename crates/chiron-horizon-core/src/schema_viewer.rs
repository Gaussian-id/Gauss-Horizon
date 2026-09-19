//! Context-aware, read-only schema metadata aggregation.
//!
//! This module is deliberately metadata-only. Adapters must not sample user
//! rows/documents/vectors/messages to infer a shape.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::connection::AppState;
use crate::connection::PoolKind;
use crate::database_manifest;
use crate::db;
use crate::document_ops::ElasticsearchIndexMetadataKind;
use crate::models::connection::DatabaseType;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum SchemaViewerKind {
    Relational,
    Document,
    Graph,
    Hybrid,
    Vector,
    Timeseries,
    WideColumn,
    KeyValue,
    Service,
    Dynamic,
}

impl SchemaViewerKind {
    fn parse(value: &str) -> Result<Self, String> {
        serde_json::from_value(Value::String(value.to_string()))
            .map_err(|_| format!("Unknown Schema Viewer kind '{value}'"))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SchemaViewerDescriptor {
    pub supported: bool,
    pub kind: SchemaViewerKind,
    pub provider: String,
    pub scope_levels: Vec<String>,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SchemaViewScope {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub catalog: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub database: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub schema: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object_kind: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SchemaScopePage {
    pub items: Vec<SchemaViewScope>,
    pub next_offset: Option<usize>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum MetadataAvailability {
    Complete,
    Partial,
    Absent,
    NotExposed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MetadataFacetStatus {
    pub name: String,
    pub availability: MetadataAvailability,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SchemaViewTable {
    pub table: db::TableInfo,
    pub columns: Vec<db::ColumnInfo>,
    pub indexes: Vec<db::IndexInfo>,
    pub foreign_keys: Vec<db::ForeignKeyInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", content = "data", rename_all = "kebab-case")]
pub enum SchemaViewPayload {
    Relational { tables: Vec<SchemaViewTable> },
    Document { metadata: Value },
    Graph { nodes: Vec<Value>, edges: Vec<Value>, metadata: Value },
    Composite { metadata: Value },
    Vector { metadata: Value },
    Timeseries { tables: Vec<SchemaViewTable> },
    WideColumn { tables: Vec<SchemaViewTable> },
    KeyValue { metadata: Value },
    Service { metadata: Value },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SchemaViewResponse {
    pub version: u32,
    pub kind: SchemaViewerKind,
    pub scope: SchemaViewScope,
    pub availability: MetadataAvailability,
    pub facets: Vec<MetadataFacetStatus>,
    pub notices: Vec<String>,
    pub payload: SchemaViewPayload,
}

async fn connection_type(state: &AppState, connection_id: &str) -> Result<DatabaseType, String> {
    state
        .configs
        .read()
        .await
        .get(connection_id)
        .map(|config| config.db_type)
        .ok_or_else(|| "Connection not found".to_string())
}

pub async fn describe_schema_viewer_core(
    state: &AppState,
    connection_id: &str,
) -> Result<SchemaViewerDescriptor, String> {
    let config = state.configs.read().await.get(connection_id).cloned().ok_or("Connection not found")?;
    let db_type = config.db_type;
    let entry = database_manifest::entry(&db_type).ok_or("Database type is not registered")?;
    if db_type == DatabaseType::Plugin {
        return match state.plugin_host.connection_schema_viewer(&config)? {
            Some(contract) => {
                let pool_key = state.get_or_create_pool(connection_id, None).await?;
                let pool = state.pool_handle(&pool_key).await.ok_or("Plugin connection pool not found")?;
                let PoolKind::PluginConnection(handle) = pool else {
                    return Err("Connection is not plugin-owned".into());
                };
                let value = handle.describe_schema_viewer().await?;
                let runtime: SchemaViewerDescriptor = serde_json::from_value(value)
                    .map_err(|error| format!("Plugin returned a malformed Schema Viewer descriptor: {error}"))?;
                let declared_kind = SchemaViewerKind::parse(&contract.kind)?;
                if runtime.supported && (runtime.kind != declared_kind || runtime.scope_levels != contract.scope_levels)
                {
                    return Err(
                        "Plugin Schema Viewer runtime descriptor does not match its manifest declaration".into()
                    );
                }
                Ok(runtime)
            }
            None => Ok(SchemaViewerDescriptor {
                supported: false,
                kind: SchemaViewerKind::Dynamic,
                provider: entry.schema_viewer.provider.clone(),
                scope_levels: entry.schema_viewer.scope_levels.clone(),
                reason: Some("The installed plugin provider does not declare Schema Viewer support.".into()),
            }),
        };
    }
    if db_type == DatabaseType::Jdbc {
        return Ok(SchemaViewerDescriptor {
            supported: true,
            kind: SchemaViewerKind::Relational,
            provider: "jdbc-database-metadata".into(),
            scope_levels: entry.schema_viewer.scope_levels.clone(),
            reason: None,
        });
    }
    Ok(SchemaViewerDescriptor {
        supported: true,
        kind: SchemaViewerKind::parse(&entry.schema_viewer.kind)?,
        provider: entry.schema_viewer.provider.clone(),
        scope_levels: entry.schema_viewer.scope_levels.clone(),
        reason: None,
    })
}

pub async fn list_schema_viewer_scopes_core(
    state: &AppState,
    connection_id: &str,
    parent: &SchemaViewScope,
    search: Option<&str>,
    limit: usize,
    offset: usize,
) -> Result<SchemaScopePage, String> {
    let descriptor = describe_schema_viewer_core(state, connection_id).await?;
    if !descriptor.supported {
        return Err(descriptor.reason.unwrap_or_else(|| "Schema Viewer is not supported".into()));
    }
    let config = state.configs.read().await.get(connection_id).cloned().ok_or("Connection not found")?;
    if config.db_type == DatabaseType::Plugin {
        let pool_key = state.get_or_create_pool(connection_id, None).await?;
        let pool = state.pool_handle(&pool_key).await.ok_or("Plugin connection pool not found")?;
        let PoolKind::PluginConnection(handle) = pool else {
            return Err("Connection is not plugin-owned".into());
        };
        let value = handle
            .list_schema_viewer_scopes(json!({
                "parentScope": parent,
                "search": search,
                "limit": limit.clamp(1, 500),
                "offset": offset,
            }))
            .await?;
        return serde_json::from_value(value)
            .map_err(|error| format!("Plugin returned malformed Schema Viewer scopes: {error}"));
    }
    let mut items = if descriptor.kind == SchemaViewerKind::KeyValue && config.db_type == DatabaseType::Etcd {
        if parent.database.is_none() {
            vec![SchemaViewScope { database: Some("default".into()), ..Default::default() }]
        } else if parent.object.is_none() {
            crate::agent_kv::kv_list_prefix_core_with_range_options(
                state,
                connection_id,
                "",
                limit.clamp(1, 500),
                None,
                crate::agent_kv::KvRangeOptions { include_values: Some(false), ..Default::default() },
            )
            .await?
            .keys
            .into_iter()
            .map(|key| SchemaViewScope {
                database: parent.database.clone(),
                object: Some(key.key),
                object_kind: Some("key".into()),
                ..Default::default()
            })
            .collect()
        } else {
            Vec::new()
        }
    } else if descriptor.kind == SchemaViewerKind::Service {
        match config.db_type {
            DatabaseType::Nacos if parent.database.is_none() => {
                crate::nacos::service::nacos_list_namespaces_core(state, connection_id)
                    .await?
                    .into_iter()
                    .map(|namespace| SchemaViewScope {
                        database: Some(namespace.namespace),
                        object_kind: Some("namespace".into()),
                        ..Default::default()
                    })
                    .collect()
            }
            DatabaseType::Consul if parent.database.is_none() => vec![SchemaViewScope {
                database: Some(config.database.clone().unwrap_or_else(|| "current catalog".into())),
                object_kind: Some("catalog".into()),
                ..Default::default()
            }],
            #[cfg(feature = "mq-admin")]
            DatabaseType::MessageQueue if parent.database.is_none() => {
                crate::mq::service::mq_list_tenants_core(state, connection_id)
                    .await?
                    .into_iter()
                    .map(|tenant| SchemaViewScope { database: Some(tenant.name), ..Default::default() })
                    .collect()
            }
            #[cfg(feature = "mq-admin")]
            DatabaseType::MessageQueue if parent.object.is_none() => crate::mq::service::mq_list_namespaces_core(
                state,
                connection_id,
                parent.database.as_deref().unwrap_or(""),
            )
            .await?
            .into_iter()
            .map(|namespace| SchemaViewScope {
                database: parent.database.clone(),
                object: Some(namespace.namespace),
                object_kind: Some("namespace".into()),
                ..Default::default()
            })
            .collect(),
            DatabaseType::Mqtt if parent.database.is_none() => {
                vec![SchemaViewScope { database: Some("broker".into()), ..Default::default() }]
            }
            DatabaseType::ZooKeeper if parent.database.is_none() => {
                vec![SchemaViewScope { database: Some("/".into()), ..Default::default() }]
            }
            _ => Vec::new(),
        }
    } else if parent.database.is_none() {
        let names = match descriptor.kind {
            SchemaViewerKind::Document => crate::document_ops::list_databases_core(state, connection_id).await,
            SchemaViewerKind::KeyValue if config.db_type == DatabaseType::Redis => {
                crate::redis_ops::redis_list_databases_core(state, connection_id)
                    .await
                    .map(|rows| rows.into_iter().map(|row| row.db.to_string()).collect())
            }
            _ => crate::schema::list_databases_core(state, connection_id)
                .await
                .map(|rows| rows.into_iter().map(|row| row.name).collect()),
        }
        .unwrap_or_else(|_| config.database.clone().into_iter().collect());
        names.into_iter().map(|database| SchemaViewScope { database: Some(database), ..Default::default() }).collect()
    } else if descriptor.scope_levels.iter().any(|level| level == "schema") && parent.schema.is_none() {
        crate::schema::list_schemas_core(state, connection_id, parent.database.as_deref().unwrap_or(""))
            .await?
            .into_iter()
            .map(|schema| SchemaViewScope {
                database: parent.database.clone(),
                schema: Some(schema),
                ..Default::default()
            })
            .collect()
    } else if descriptor.scope_levels.iter().any(|level| level == "object") && parent.object.is_none() {
        let database = parent.database.as_deref().unwrap_or("");
        let schema = parent.schema.as_deref().unwrap_or("");
        match descriptor.kind {
            SchemaViewerKind::Document => crate::document_ops::list_collections_core(state, connection_id, database)
                .await?
                .into_iter()
                .map(|collection| SchemaViewScope {
                    database: parent.database.clone(),
                    object: Some(collection.name),
                    object_kind: collection.kind,
                    ..Default::default()
                })
                .collect(),
            SchemaViewerKind::Vector | SchemaViewerKind::Hybrid => {
                crate::schema::list_vector_collections_core(state, connection_id, database)
                    .await?
                    .into_iter()
                    .map(|collection| SchemaViewScope {
                        database: parent.database.clone(),
                        object: Some(collection.name),
                        object_kind: Some("collection".into()),
                        ..Default::default()
                    })
                    .collect()
            }
            SchemaViewerKind::KeyValue if config.db_type == DatabaseType::Redis => {
                let db = database.parse::<u32>().unwrap_or(0);
                crate::redis_ops::redis_scan_keys_batch_core(
                    state,
                    connection_id,
                    db,
                    0,
                    "*",
                    limit.clamp(1, 500),
                    1,
                    true,
                )
                .await?
                .keys
                .into_iter()
                .map(|key| SchemaViewScope {
                    database: parent.database.clone(),
                    object: Some(key.key_display),
                    object_kind: Some(key.key_type),
                    ..Default::default()
                })
                .collect()
            }
            _ => crate::schema::list_tables_core(state, connection_id, database, schema, None, None, None, None, None)
                .await?
                .into_iter()
                .map(|table| SchemaViewScope {
                    database: parent.database.clone(),
                    schema: parent.schema.clone(),
                    object: Some(table.name),
                    object_kind: Some(table.table_type),
                    ..Default::default()
                })
                .collect(),
        }
    } else {
        Vec::new()
    };
    if let Some(search) = search.map(str::trim).filter(|search| !search.is_empty()) {
        let search = search.to_lowercase();
        items.retain(|item| {
            item.database
                .as_deref()
                .or(item.schema.as_deref())
                .or(item.object.as_deref())
                .is_some_and(|value| value.to_lowercase().contains(&search))
        });
    }
    let total = items.len();
    let items = items.into_iter().skip(offset).take(limit.clamp(1, 500)).collect::<Vec<_>>();
    let next_offset = (offset + items.len() < total).then_some(offset + items.len());
    Ok(SchemaScopePage { items, next_offset })
}

async fn load_tables(
    state: &AppState,
    connection_id: &str,
    scope: &SchemaViewScope,
) -> Result<(Vec<SchemaViewTable>, Vec<MetadataFacetStatus>), String> {
    let database = scope.database.as_deref().unwrap_or("");
    let schema = scope.schema.as_deref().unwrap_or("");
    let chirondb_relational =
        state.configs.read().await.get(connection_id).is_some_and(crate::schema::is_chirondb_relational_config);
    let mut tables =
        crate::schema::list_tables_core(state, connection_id, database, schema, None, None, None, None, None).await?;
    if let Some(object) = scope.object.as_deref() {
        tables.retain(|table| table.name == object);
    }
    let mut output = Vec::with_capacity(tables.len());
    let mut facets =
        vec![MetadataFacetStatus { name: "tables".into(), availability: MetadataAvailability::Complete, error: None }];
    let mut columns_error = None;
    let mut indexes_error = chirondb_relational.then(|| {
        "ChironDB's PostgreSQL-wire preview does not yet expose complete index metadata to Horizon.".to_string()
    });
    let mut foreign_keys_error = chirondb_relational.then(|| {
        "ChironDB's PostgreSQL-wire preview does not yet expose complete foreign-key metadata to Horizon.".to_string()
    });
    for table in tables {
        let columns = match crate::schema::get_columns_core(state, connection_id, database, schema, &table.name).await {
            Ok(value) => value,
            Err(error) if metadata_failure_is_fatal(&error) => return Err(error),
            Err(error) => {
                columns_error.get_or_insert(error);
                Vec::new()
            }
        };
        let indexes = if chirondb_relational {
            Vec::new()
        } else {
            match crate::schema::list_indexes_core(state, connection_id, database, schema, &table.name).await {
                Ok(value) => value,
                Err(error) if metadata_failure_is_fatal(&error) => return Err(error),
                Err(error) => {
                    indexes_error.get_or_insert(error);
                    Vec::new()
                }
            }
        };
        let foreign_keys = if chirondb_relational {
            Vec::new()
        } else {
            match crate::schema::list_foreign_keys_core(state, connection_id, database, schema, &table.name).await {
                Ok(value) => value,
                Err(error) if metadata_failure_is_fatal(&error) => return Err(error),
                Err(error) => {
                    foreign_keys_error.get_or_insert(error);
                    Vec::new()
                }
            }
        };
        output.push(SchemaViewTable { table, columns, indexes, foreign_keys });
    }
    for (name, error) in [("columns", columns_error), ("indexes", indexes_error), ("foreignKeys", foreign_keys_error)] {
        facets.push(MetadataFacetStatus {
            name: name.into(),
            availability: if error.is_some() { MetadataAvailability::Partial } else { MetadataAvailability::Complete },
            error,
        });
    }
    Ok((output, facets))
}

/// Unsupported metadata surfaces may degrade one facet to `partial`, but an
/// authorization or transport failure must remain an error. Treating those as
/// an empty schema would be misleading and could conceal an operational fault.
fn metadata_failure_is_fatal(error: &str) -> bool {
    let error = error.to_ascii_lowercase();
    [
        "permission denied",
        "access denied",
        "not authorized",
        "unauthorized",
        "forbidden",
        "authentication",
        "connection refused",
        "connection reset",
        "connection closed",
        "timed out",
        "timeout",
        "transport",
        "network error",
        "broken pipe",
    ]
    .iter()
    .any(|needle| error.contains(needle))
}

async fn neo4j_relationship_types(state: &AppState, connection_id: &str, database: &str) -> Result<Vec<Value>, String> {
    let pool_key = state
        .get_or_create_metadata_pool_for_session(connection_id, (!database.is_empty()).then_some(database), None)
        .await?;
    let pool = state.pool_handle(&pool_key).await.ok_or("Neo4j metadata pool not found")?;
    let PoolKind::Agent(client) = pool else {
        return Err("Neo4j graph metadata requires an Agent connection".to_string());
    };
    let mut client = client.lock().await;
    let result = client
        .execute_query_with_timeout::<db::QueryResult>(
            crate::query::agent_execute_query_params(
                "CALL db.relationshipTypes() YIELD relationshipType RETURN relationshipType ORDER BY relationshipType",
                (!database.is_empty()).then_some(database),
                None,
                crate::query::QueryExecutionOptions { max_rows: Some(10_000), ..Default::default() },
            ),
            Some(std::time::Duration::from_secs(60)),
        )
        .await?;
    Ok(result
        .rows
        .into_iter()
        .filter_map(|row| row.into_iter().next())
        .filter_map(|value| value.as_str().map(|name| json!({ "type": name, "source": null, "target": null })))
        .collect())
}

async fn neo4j_metadata_query(
    state: &AppState,
    connection_id: &str,
    database: &str,
    sql: &str,
) -> Result<db::QueryResult, String> {
    let pool_key = state
        .get_or_create_metadata_pool_for_session(connection_id, (!database.is_empty()).then_some(database), None)
        .await?;
    let pool = state.pool_handle(&pool_key).await.ok_or("Neo4j metadata pool not found")?;
    let PoolKind::Agent(client) = pool else {
        return Err("Neo4j graph metadata requires an Agent connection".to_string());
    };
    let mut client = client.lock().await;
    client
        .execute_query_with_timeout::<db::QueryResult>(
            crate::query::agent_execute_query_params(
                sql,
                (!database.is_empty()).then_some(database),
                None,
                crate::query::QueryExecutionOptions { max_rows: Some(10_000), ..Default::default() },
            ),
            Some(std::time::Duration::from_secs(60)),
        )
        .await
}

fn neo4j_schema_value(value: &Value) -> Value {
    value.as_str().and_then(|encoded| serde_json::from_str(encoded).ok()).unwrap_or_else(|| value.clone())
}

fn normalize_neo4j_schema_visualization(result: db::QueryResult) -> Result<(Vec<Value>, Vec<Value>), String> {
    let row = result.rows.into_iter().next().ok_or("Neo4j schema visualization returned no metadata")?;
    let raw_nodes = row.first().map(neo4j_schema_value).unwrap_or(Value::Array(Vec::new()));
    let raw_edges = row.get(1).map(neo4j_schema_value).unwrap_or(Value::Array(Vec::new()));
    let nodes = raw_nodes
        .as_array()
        .ok_or("Neo4j schema visualization returned malformed nodes")?
        .iter()
        .filter_map(|node| {
            let node = node.as_object()?;
            let id = node.get("elementId").or_else(|| node.get("id"))?.as_str()?.to_string();
            let labels = node.get("labels").and_then(Value::as_array).cloned().unwrap_or_default();
            let label = labels.iter().filter_map(Value::as_str).collect::<Vec<_>>().join(":");
            let properties = node
                .get("properties")
                .and_then(Value::as_object)
                .map(|properties| properties.keys().cloned().map(Value::String).collect::<Vec<_>>())
                .unwrap_or_default();
            Some(json!({"id": id, "label": label, "labels": labels, "properties": properties}))
        })
        .collect();
    let edges = raw_edges
        .as_array()
        .ok_or("Neo4j schema visualization returned malformed relationships")?
        .iter()
        .filter_map(|edge| {
            let edge = edge.as_object()?;
            Some(json!({
                "type": edge.get("type").and_then(Value::as_str)?,
                "source": edge.get("startElementId").or_else(|| edge.get("start"))?.as_str()?,
                "target": edge.get("endElementId").or_else(|| edge.get("end"))?.as_str()?,
                "properties": edge.get("properties").cloned().unwrap_or_else(|| json!({})),
            }))
        })
        .collect();
    Ok((nodes, edges))
}

async fn neo4j_schema_visualization(
    state: &AppState,
    connection_id: &str,
    database: &str,
) -> Result<(Vec<Value>, Vec<Value>), String> {
    normalize_neo4j_schema_visualization(
        neo4j_metadata_query(
            state,
            connection_id,
            database,
            "CALL db.schema.visualization() YIELD nodes, relationships RETURN nodes, relationships",
        )
        .await?,
    )
}

pub async fn get_schema_view_core(
    state: &AppState,
    connection_id: &str,
    scope: SchemaViewScope,
) -> Result<SchemaViewResponse, String> {
    let descriptor = describe_schema_viewer_core(state, connection_id).await?;
    if !descriptor.supported {
        return Err(descriptor.reason.unwrap_or_else(|| "Schema Viewer is not supported".into()));
    }
    let db_type = connection_type(state, connection_id).await?;
    if db_type == DatabaseType::Plugin {
        let pool_key = state.get_or_create_pool(connection_id, None).await?;
        let pool = state.pool_handle(&pool_key).await.ok_or("Plugin connection pool not found")?;
        let PoolKind::PluginConnection(handle) = pool else {
            return Err("Connection is not plugin-owned".into());
        };
        let value = handle.get_schema_view(json!({ "scope": scope })).await?;
        let response: SchemaViewResponse = serde_json::from_value(value)
            .map_err(|error| format!("Plugin returned a malformed Schema Viewer response: {error}"))?;
        if response.version != 1 || response.kind != descriptor.kind {
            return Err("Plugin Schema Viewer response does not match the negotiated contract".into());
        }
        return Ok(response);
    }
    let mut notices = Vec::new();
    let (availability, facets, payload) = match descriptor.kind {
        SchemaViewerKind::Relational => {
            let (tables, facets) = load_tables(state, connection_id, &scope).await?;
            if facets
                .iter()
                .any(|facet| facet.name == "foreignKeys" && facet.availability == MetadataAvailability::Partial)
            {
                notices.push("Tables and columns are shown, but this driver did not expose complete foreign-key metadata; the ERD may omit relationships.".into());
            }
            if facets.iter().any(|facet| facet.name == "indexes" && facet.availability == MetadataAvailability::Partial)
            {
                notices.push("Index metadata is unavailable or incomplete for this scope.".into());
            }
            let availability = if facets.iter().any(|facet| facet.availability == MetadataAvailability::Partial) {
                MetadataAvailability::Partial
            } else {
                MetadataAvailability::Complete
            };
            (availability, facets, SchemaViewPayload::Relational { tables })
        }
        SchemaViewerKind::Document => {
            let object = scope.object.as_deref().ok_or("Select a collection or index")?;
            let mut declared_error = None;
            let mut metadata = match db_type {
                DatabaseType::DynamoDb => serde_json::to_value(
                    crate::document_ops::describe_dynamodb_table_core(state, connection_id, object).await?,
                )
                .map_err(|error| error.to_string())?,
                DatabaseType::Elasticsearch | DatabaseType::Easysearch => json!({
                    "mapping": crate::document_ops::elasticsearch_get_index_metadata_core(state, connection_id, object, ElasticsearchIndexMetadataKind::Mapping).await?,
                    "settings": crate::document_ops::elasticsearch_get_index_metadata_core(state, connection_id, object, ElasticsearchIndexMetadataKind::Settings).await?,
                    "aliases": crate::document_ops::list_collections_core(state, connection_id, scope.database.as_deref().unwrap_or("default")).await?
                        .into_iter()
                        .find(|index| index.name == object)
                        .map(|index| index.aliases)
                        .unwrap_or_default(),
                }),
                DatabaseType::Meilisearch => {
                    crate::document_ops::meilisearch_get_index_settings_core(state, connection_id, object).await?
                }
                DatabaseType::MongoDb => {
                    match crate::document_ops::mongodb_get_collection_declared_schema_core(
                        state,
                        connection_id,
                        scope.database.as_deref().unwrap_or(""),
                        object,
                    )
                    .await
                    {
                        Ok(metadata) => metadata,
                        Err(error) if metadata_failure_is_fatal(&error) => return Err(error),
                        Err(error) => {
                            declared_error = Some(error);
                            Value::Null
                        }
                    }
                }
                _ => Value::Null,
            };
            let has_declared_schema = if db_type == DatabaseType::MongoDb {
                metadata.pointer("/options/validator").is_some_and(|value| !value.is_null())
            } else {
                !metadata.is_null()
            };
            let mut facets = vec![MetadataFacetStatus {
                name: "declaredSchema".into(),
                availability: if declared_error.is_some() {
                    MetadataAvailability::NotExposed
                } else if has_declared_schema {
                    MetadataAvailability::Complete
                } else {
                    MetadataAvailability::Absent
                },
                error: declared_error.clone(),
            }];
            if db_type == DatabaseType::MongoDb {
                let search_indexes = crate::document_ops::mongodb_list_collection_search_indexes_core(
                    state,
                    connection_id,
                    scope.database.as_deref().unwrap_or(""),
                    object,
                )
                .await;
                match search_indexes {
                    Ok(indexes) => {
                        if let Some(metadata) = metadata.as_object_mut() {
                            metadata.insert("searchIndexes".into(), indexes);
                        }
                        facets.push(MetadataFacetStatus {
                            name: "searchIndexes".into(),
                            availability: MetadataAvailability::Complete,
                            error: None,
                        });
                    }
                    Err(error) if metadata_failure_is_fatal(&error) => return Err(error),
                    Err(error) => facets.push(MetadataFacetStatus {
                        name: "searchIndexes".into(),
                        availability: MetadataAvailability::NotExposed,
                        error: Some(error),
                    }),
                }
            }
            if db_type == DatabaseType::MongoDb && !has_declared_schema && declared_error.is_none() {
                notices.push("This collection has no declared validator. Collection metadata is shown, and documents were not sampled.".into());
            }
            let not_exposed_count =
                facets.iter().filter(|facet| facet.availability == MetadataAvailability::NotExposed).count();
            let availability = if not_exposed_count == facets.len() {
                MetadataAvailability::NotExposed
            } else if not_exposed_count > 0 {
                MetadataAvailability::Partial
            } else if has_declared_schema {
                MetadataAvailability::Complete
            } else {
                MetadataAvailability::Absent
            };
            (availability, facets, SchemaViewPayload::Document { metadata })
        }
        SchemaViewerKind::Vector => {
            let object = scope.object.as_deref().ok_or("Select a collection")?;
            let metadata = serde_json::to_value(
                crate::schema::get_vector_collection_detail_core(
                    state,
                    connection_id,
                    scope.database.as_deref().unwrap_or(""),
                    object,
                )
                .await?,
            )
            .map_err(|error| error.to_string())?;
            let index_error = metadata
                .pointer("/nativeMetadata/data/schemaViewerIndexDetailsError")
                .and_then(Value::as_str)
                .map(str::to_string);
            let availability =
                if index_error.is_some() { MetadataAvailability::Partial } else { MetadataAvailability::Complete };
            let facets = vec![MetadataFacetStatus { name: "vectorIndexes".into(), availability, error: index_error }];
            (availability, facets, SchemaViewPayload::Vector { metadata })
        }
        SchemaViewerKind::Hybrid => {
            let object = scope.object.as_deref().ok_or("Select a collection")?;
            let reply = crate::db::chirondb::run(
                state,
                connection_id,
                crate::db::chirondb::Request::Metadata { collection: object.to_string() },
            )
            .await?;
            if !(200..300).contains(&reply.status) {
                return Err(format!("ChironDB metadata request failed (HTTP {})", reply.status));
            }
            (MetadataAvailability::Complete, vec![], SchemaViewPayload::Composite { metadata: reply.body })
        }
        SchemaViewerKind::KeyValue if db_type == DatabaseType::Redis => {
            let database = scope.database.as_deref().unwrap_or("0").parse::<u32>().unwrap_or(0);
            let page =
                crate::redis_ops::redis_scan_keys_batch_core(state, connection_id, database, 0, "*", 200, 1, true)
                    .await?;
            (
                MetadataAvailability::Complete,
                vec![],
                SchemaViewPayload::KeyValue {
                    metadata: serde_json::to_value(page).map_err(|error| error.to_string())?,
                },
            )
        }
        SchemaViewerKind::KeyValue if db_type == DatabaseType::Etcd => {
            let prefix = scope.object.as_deref().unwrap_or("");
            let mut page = crate::agent_kv::kv_list_prefix_core_with_range_options(
                state,
                connection_id,
                prefix,
                500,
                None,
                crate::agent_kv::KvRangeOptions { include_values: Some(false), ..Default::default() },
            )
            .await?;
            // Keep this defensive even for older Agents which ignore the
            // includeValues flag: Schema Viewer never returns KV values.
            for key in &mut page.keys {
                key.value = None;
            }
            (
                MetadataAvailability::Complete,
                vec![],
                SchemaViewPayload::KeyValue {
                    metadata: serde_json::to_value(page).map_err(|error| error.to_string())?,
                },
            )
        }
        SchemaViewerKind::Graph => {
            let database = scope.database.as_deref().unwrap_or("");
            let mut graph_facets = Vec::new();
            let (nodes, edges) = match neo4j_schema_visualization(state, connection_id, database).await {
                Ok(graph) => {
                    graph_facets.push(MetadataFacetStatus {
                        name: "topology".into(),
                        availability: MetadataAvailability::Complete,
                        error: None,
                    });
                    graph
                }
                Err(error) if metadata_failure_is_fatal(&error) => return Err(error),
                Err(error) => {
                    graph_facets.push(MetadataFacetStatus {
                        name: "topology".into(),
                        availability: MetadataAvailability::Partial,
                        error: Some(error),
                    });
                    let labels = neo4j_metadata_query(
                        state,
                        connection_id,
                        database,
                        "CALL db.labels() YIELD label RETURN label ORDER BY label",
                    )
                    .await?;
                    let nodes = labels
                        .rows
                        .into_iter()
                        .filter_map(|row| row.into_iter().next())
                        .filter_map(|value| {
                            value.as_str().map(|label| json!({"id": label, "label": label, "properties": []}))
                        })
                        .collect();
                    let edges = match neo4j_relationship_types(state, connection_id, database).await {
                        Ok(edges) => edges,
                        Err(fallback_error) if metadata_failure_is_fatal(&fallback_error) => {
                            return Err(fallback_error)
                        }
                        Err(fallback_error) => {
                            graph_facets.push(MetadataFacetStatus {
                                name: "relationshipTypes".into(),
                                availability: MetadataAvailability::Partial,
                                error: Some(fallback_error),
                            });
                            Vec::new()
                        }
                    };
                    (nodes, edges)
                }
            };
            let mut native = serde_json::Map::new();
            for (name, query) in [
                ("nodeTypeProperties", "CALL db.schema.nodeTypeProperties()"),
                ("relationshipTypeProperties", "CALL db.schema.relTypeProperties()"),
                ("indexes", "SHOW INDEXES"),
                ("constraints", "SHOW CONSTRAINTS"),
            ] {
                match neo4j_metadata_query(state, connection_id, database, query).await {
                    Ok(result) => {
                        native.insert(name.into(), serde_json::to_value(result).map_err(|error| error.to_string())?);
                        graph_facets.push(MetadataFacetStatus {
                            name: name.into(),
                            availability: MetadataAvailability::Complete,
                            error: None,
                        });
                    }
                    Err(error) if metadata_failure_is_fatal(&error) => return Err(error),
                    Err(error) => {
                        native.insert(name.into(), Value::Null);
                        graph_facets.push(MetadataFacetStatus {
                            name: name.into(),
                            availability: MetadataAvailability::Partial,
                            error: Some(error),
                        });
                    }
                }
            }
            if edges.iter().any(|edge| edge.get("source").is_some_and(Value::is_null)) {
                notices.push("Relationship types are shown, but this Neo4j version did not expose source/target schema endpoints without inspecting graph data.".into());
            }
            let availability = if graph_facets.iter().any(|facet| facet.availability == MetadataAvailability::Partial) {
                MetadataAvailability::Partial
            } else {
                MetadataAvailability::Complete
            };
            (availability, graph_facets, SchemaViewPayload::Graph { nodes, edges, metadata: Value::Object(native) })
        }
        SchemaViewerKind::Timeseries => {
            let (tables, facets) = load_tables(state, connection_id, &scope).await?;
            let availability = if facets.iter().any(|facet| facet.availability == MetadataAvailability::Partial) {
                MetadataAvailability::Partial
            } else {
                MetadataAvailability::Complete
            };
            (availability, facets, SchemaViewPayload::Timeseries { tables })
        }
        SchemaViewerKind::WideColumn => {
            let (tables, facets) = load_tables(state, connection_id, &scope).await?;
            let availability = if facets.iter().any(|facet| facet.availability == MetadataAvailability::Partial) {
                MetadataAvailability::Partial
            } else {
                MetadataAvailability::Complete
            };
            (availability, facets, SchemaViewPayload::WideColumn { tables })
        }
        SchemaViewerKind::KeyValue => {
            notices.push(
                "The active key-value protocol does not expose additional schema metadata for this scope.".into(),
            );
            (
                MetadataAvailability::NotExposed,
                vec![],
                SchemaViewPayload::KeyValue { metadata: json!({"scope": scope}) },
            )
        }
        SchemaViewerKind::Service => {
            notices.push("Only protocol metadata is displayed. Message payloads, configuration bodies, secrets, and registry values are never read.".into());
            let metadata = match db_type {
                DatabaseType::Nacos => {
                    let mut namespaces =
                        crate::nacos::service::nacos_list_namespaces_core(state, connection_id).await?;
                    if let Some(namespace) = scope.database.as_deref() {
                        namespaces.retain(|item| item.namespace == namespace);
                    }
                    json!({"namespaces": namespaces})
                }
                DatabaseType::Consul => {
                    let datacenter = scope.database.clone();
                    let services = crate::consul::consul_catalog_services_core(
                        state,
                        connection_id,
                        crate::consul::ConsulReadOptions::default(),
                    )
                    .await?;
                    json!({
                        "datacenter": datacenter,
                        "datacenters": crate::consul::consul_catalog_datacenters_core(state, connection_id).await?,
                        "services": services.items,
                        "responseMetadata": services.metadata
                    })
                }
                #[cfg(feature = "mq-admin")]
                DatabaseType::MessageQueue => {
                    let cluster = crate::mq::service::mq_test_connection_core(state, connection_id).await?;
                    let topics =
                        if let (Some(tenant), Some(namespace)) = (scope.database.as_deref(), scope.object.as_deref()) {
                            crate::mq::service::mq_list_topics_core(
                                state,
                                connection_id,
                                crate::mq::types::NamespaceRef {
                                    tenant: tenant.to_string(),
                                    namespace: namespace.to_string(),
                                },
                                crate::mq::types::ListTopicsOpts::default(),
                            )
                            .await?
                        } else {
                            Vec::new()
                        };
                    json!({"cluster": cluster, "topics": topics})
                }
                #[cfg(not(feature = "mq-admin"))]
                DatabaseType::MessageQueue => {
                    return Err("Message queue metadata support is not compiled in this build".into());
                }
                DatabaseType::Mqtt => {
                    #[cfg(feature = "mq-admin")]
                    {
                        let pool_key = state.get_or_create_pool(connection_id, None).await?;
                        let pool = state.pool_handle(&pool_key).await.ok_or("MQTT connection pool not found")?;
                        let PoolKind::Mqtt(client) = pool else {
                            return Err("Connection is not an MQTT broker".into());
                        };
                        json!({
                            "broker": crate::mqtt::service::get_broker_info(&client).await?,
                            "subscriptions": crate::mqtt::service::list_topics(&client).await?,
                        })
                    }
                    #[cfg(not(feature = "mq-admin"))]
                    {
                        return Err("MQTT support is not compiled in this build".into());
                    }
                }
                DatabaseType::ZooKeeper => {
                    let mut page = crate::agent_kv::kv_list_prefix_core_with_range_options(
                        state,
                        connection_id,
                        scope.object.as_deref().unwrap_or("/"),
                        500,
                        None,
                        crate::agent_kv::KvRangeOptions { include_values: Some(false), ..Default::default() },
                    )
                    .await?;
                    for key in &mut page.keys {
                        key.value = None;
                    }
                    serde_json::to_value(page).map_err(|error| error.to_string())?
                }
                _ => json!({"databaseType": db_type.as_str(), "scope": scope}),
            };
            (MetadataAvailability::Complete, vec![], SchemaViewPayload::Service { metadata })
        }
        SchemaViewerKind::Dynamic => {
            return Err("The runtime provider did not negotiate a concrete Schema Viewer payload kind".into());
        }
    };
    Ok(SchemaViewResponse { version: 1, kind: descriptor.kind, scope, availability, facets, notices, payload })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_database_type_has_a_schema_viewer_contract() {
        for db_type in DatabaseType::ALL {
            let entry = database_manifest::entry(db_type).expect("registered database type");
            assert!(SchemaViewerKind::parse(&entry.schema_viewer.kind).is_ok(), "{}", db_type.as_str());
            assert!(!entry.schema_viewer.scope_levels.is_empty(), "{}", db_type.as_str());
            assert!(entry.access.connect_supported && entry.access.test_connection_supported, "{}", db_type.as_str());
        }
    }

    #[test]
    fn permission_and_transport_failures_are_not_downgraded_to_partial_metadata() {
        for error in [
            "permission denied for relation pg_indexes",
            "not authorized to execute metadata command",
            "connection refused",
            "request timed out",
        ] {
            assert!(metadata_failure_is_fatal(error), "{error}");
        }
        assert!(!metadata_failure_is_fatal("indexes are not supported by this driver"));
    }

    #[test]
    fn neo4j_schema_visualization_preserves_native_node_edge_topology() {
        let result =
            db::QueryResult {
                columns: vec!["nodes".into(), "relationships".into()],
                column_types: vec!["Array".into(), "Array".into()],
                rows: vec![vec![
                Value::String(json!([
                    {"elementId":"n1","labels":["Person"],"properties":{"name":"String"}},
                    {"elementId":"n2","labels":["Company"],"properties":{}}
                ]).to_string()),
                Value::String(json!([
                    {"elementId":"r1","startElementId":"n1","endElementId":"n2","type":"WORKS_AT","properties":{}}
                ]).to_string()),
            ]],
                column_sortables: vec![],
                spatial_columns: vec![],
                spatial_values: vec![],
                affected_rows: 0,
                execution_time_ms: 0,
                truncated: false,
                session_id: None,
                has_more: false,
                elasticsearch_raw_body: None,
                messages: vec![],
            };
        let (nodes, edges) = normalize_neo4j_schema_visualization(result).unwrap();
        assert_eq!(nodes[0]["label"], "Person");
        assert_eq!(nodes[0]["properties"], json!(["name"]));
        assert_eq!(edges[0]["source"], "n1");
        assert_eq!(edges[0]["target"], "n2");
    }

    #[test]
    fn malformed_or_unnegotiated_transport_metadata_is_rejected() {
        assert!(serde_json::from_value::<SchemaViewResponse>(json!({
            "version": 1,
            "kind": "document",
            "scope": {},
            "availability": "unavailable",
            "facets": [],
            "notices": [],
            "payload": {"kind": "document", "data": {}}
        }))
        .is_err());
        assert!(serde_json::from_value::<SchemaViewResponse>(json!({
            "version": 1,
            "kind": "dynamic",
            "scope": {},
            "availability": "notExposed",
            "facets": [],
            "notices": [],
            "payload": {"kind": "dynamic", "data": {}}
        }))
        .is_err());
    }

    #[test]
    fn every_payload_variant_round_trips_through_the_transport_contract() {
        let cases = [
            (SchemaViewerKind::Relational, SchemaViewPayload::Relational { tables: vec![] }),
            (SchemaViewerKind::Graph, SchemaViewPayload::Graph { nodes: vec![], edges: vec![], metadata: json!({}) }),
            (SchemaViewerKind::Document, SchemaViewPayload::Document { metadata: json!({}) }),
            (SchemaViewerKind::Hybrid, SchemaViewPayload::Composite { metadata: json!({}) }),
            (SchemaViewerKind::Vector, SchemaViewPayload::Vector { metadata: json!({}) }),
            (SchemaViewerKind::Timeseries, SchemaViewPayload::Timeseries { tables: vec![] }),
            (SchemaViewerKind::WideColumn, SchemaViewPayload::WideColumn { tables: vec![] }),
            (SchemaViewerKind::KeyValue, SchemaViewPayload::KeyValue { metadata: json!({}) }),
            (SchemaViewerKind::Service, SchemaViewPayload::Service { metadata: json!({}) }),
        ];
        for (kind, payload) in cases {
            let response = SchemaViewResponse {
                version: 1,
                kind,
                scope: SchemaViewScope::default(),
                availability: MetadataAvailability::Complete,
                facets: vec![],
                notices: vec![],
                payload,
            };
            let value = serde_json::to_value(&response).unwrap();
            let decoded: SchemaViewResponse = serde_json::from_value(value).unwrap();
            assert_eq!(decoded.version, 1);
            assert_eq!(decoded.kind, response.kind);
        }
    }
}
