import type { DatabaseType } from "@/types/database";
import { databaseSchemaViewerContract, type SchemaViewerKind, type SchemaViewerScopeLevel } from "@/lib/database/databaseDriverManifest";
import type { ColumnInfo, ForeignKeyInfo, IndexInfo, TableInfo } from "@/types/database";

export type { SchemaViewerKind, SchemaViewerScopeLevel };

export type MetadataAvailability = "complete" | "partial" | "absent" | "notExposed";

export interface SchemaViewerDescriptor {
  supported: boolean;
  kind: SchemaViewerKind;
  provider: string;
  scopeLevels: SchemaViewerScopeLevel[];
  reason?: string | null;
}

export interface SchemaViewScope {
  catalog?: string;
  database?: string;
  schema?: string;
  object?: string;
  objectKind?: string;
}

export interface SchemaScopePage {
  items: SchemaViewScope[];
  nextOffset?: number | null;
}

export interface SchemaViewTable {
  table: TableInfo;
  columns: ColumnInfo[];
  indexes: IndexInfo[];
  foreignKeys: ForeignKeyInfo[];
}

export type SchemaViewPayloadKind = "relational" | "graph" | "document" | "vector" | "timeseries" | "wide-column" | "key-value" | "service" | "composite";

export interface SchemaViewResponse {
  version: number;
  kind: SchemaViewerKind;
  scope: SchemaViewScope;
  availability: MetadataAvailability;
  facets: Array<{ name: string; availability: MetadataAvailability; error?: string }>;
  notices: string[];
  payload: { kind: SchemaViewPayloadKind; data: Record<string, unknown> };
}

/**
 * A storage-model classification for presentation only. It deliberately does
 * not infer anything from user rows: metadata loading is handled by the
 * selected renderer and remains driver-native.
 */
export function schemaViewerKind(dbType: DatabaseType): SchemaViewerKind {
  const contract = databaseSchemaViewerContract(dbType);
  if (!contract) throw new Error(`Database type ${dbType} has no Schema Viewer contract`);
  return contract.kind;
}

export function schemaViewerScopeLevels(dbType: DatabaseType): SchemaViewerScopeLevel[] {
  return databaseSchemaViewerContract(dbType)?.scopeLevels ?? [];
}

export function schemaViewerKindLabel(kind: SchemaViewerKind): string {
  return {
    relational: "Relational ERD",
    graph: "Metagraph",
    document: "Document schema",
    hybrid: "Hybrid schema",
    vector: "Vector summary",
    timeseries: "Time-series summary",
    "wide-column": "Wide-column schema",
    "key-value": "Key-value metadata",
    service: "Service metadata",
    dynamic: "Runtime schema",
  }[kind];
}

export function schemaViewerScopeLabel(kind: SchemaViewerKind): string {
  if (kind === "document" || kind === "vector" || kind === "hybrid") return "Collection / index";
  if (kind === "graph") return "Graph / database";
  if (kind === "timeseries") return "Measurement / table";
  if (kind === "wide-column") return "Table / column family";
  if (kind === "key-value") return "Namespace / key";
  if (kind === "service") return "Namespace / resource";
  return "Database / schema";
}
