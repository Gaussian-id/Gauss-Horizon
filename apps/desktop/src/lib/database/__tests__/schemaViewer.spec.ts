import { describe, expect, it } from "vitest";
import { readFileSync } from "node:fs";
import { supportsSchemaViewer } from "@/lib/database/databaseFeatureSupport";
import { schemaViewerKind, schemaViewerScopeLabel } from "@/lib/database/schemaViewer";
import { DATABASE_TYPES } from "@/types/generated/databaseTypes";
import { databaseAccessContract, databaseSchemaViewerContract, manifestDatabaseTypes } from "@/lib/database/databaseDriverManifest";
import { CONNECTION_PICKER_OPTIONS, CONNECTION_PROFILES } from "@/types/generated/connectionProfiles";

describe("context-aware schema viewer", () => {
  it("uses the storage model instead of the legacy ERD flag", () => {
    expect(schemaViewerKind("postgres")).toBe("relational");
    expect(schemaViewerKind("neo4j")).toBe("graph");
    expect(schemaViewerKind("chirondb")).toBe("hybrid");
    expect(schemaViewerKind("mongodb")).toBe("document");
    expect(schemaViewerKind("milvus")).toBe("vector");
    expect(schemaViewerKind("influxdb3")).toBe("timeseries");
  });

  it("exposes the viewer only through explicit manifest capabilities", () => {
    expect(supportsSchemaViewer("postgres")).toBe(true);
    expect(supportsSchemaViewer("neo4j")).toBe(true);
    expect(supportsSchemaViewer("mongodb")).toBe(true);
    expect(supportsSchemaViewer("elasticsearch")).toBe(true);
    expect(supportsSchemaViewer("influxdb")).toBe(true);
    expect(supportsSchemaViewer("redis")).toBe(true);
  });

  it("keeps the default-enabled navbar action behind the saved toolbar toggle", () => {
    const toolbar = readFileSync(new URL("../../../components/layout/AppToolbar.vue", import.meta.url), "utf8");
    expect(toolbar).toContain('v-if="toolbarItems.schemaViewer"');
    expect(toolbar).toContain("emit('open-schema-viewer')");
  });

  it("uses collection scope only for collection-oriented stores", () => {
    expect(schemaViewerScopeLabel("document")).toBe("Collection / index");
    expect(schemaViewerScopeLabel("vector")).toBe("Collection / index");
    expect(schemaViewerScopeLabel("relational")).toBe("Database / schema");
  });

  it("has explicit access and schema contracts for all 82 registered types", () => {
    expect(DATABASE_TYPES).toHaveLength(82);
    expect(new Set(manifestDatabaseTypes())).toEqual(new Set(DATABASE_TYPES));
    for (const dbType of DATABASE_TYPES) {
      expect(databaseAccessContract(dbType)).toMatchObject({ connectSupported: true, testConnectionSupported: true });
      expect(databaseSchemaViewerContract(dbType)?.scopeLevels.length).toBeGreaterThan(0);
      expect(supportsSchemaViewer(dbType)).toBe(true);
    }
  });

  it("keeps every built-in type reachable directly or through its declared merged/version profile", () => {
    const visibleProfiles = new Set(CONNECTION_PICKER_OPTIONS.map((option) => option.value));
    for (const dbType of DATABASE_TYPES) {
      const access = databaseAccessContract(dbType)!;
      if (access.connectionPicker === "plugin-provided") {
        expect(dbType).toBe("plugin");
        continue;
      }
      const profiles = Object.entries(CONNECTION_PROFILES).filter(([, profile]) => profile.type === dbType);
      expect(profiles.length, dbType).toBeGreaterThan(0);
      if (access.connectionPicker === "direct") {
        expect(
          profiles.some(([id]) => visibleProfiles.has(id)),
          dbType,
        ).toBe(true);
      }
    }
    expect(CONNECTION_PROFILES.ignite3.type).toBe("ignite3");
    expect(CONNECTION_PROFILES.influxdb3.type).toBe("influxdb3");
    expect(CONNECTION_PROFILES.doris.type).toBe("doris");
    expect(CONNECTION_PROFILES.starrocks.type).toBe("starrocks");
    expect([CONNECTION_PROFILES.mq.type, CONNECTION_PROFILES.kafka.type, CONNECTION_PROFILES.rabbitmq.type, CONNECTION_PROFILES.rocketmq.type]).toEqual(["mq", "mq", "mq", "mq"]);
  });
});
