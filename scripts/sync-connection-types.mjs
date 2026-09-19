import { existsSync, mkdirSync, readdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import process from "node:process";
import { parse } from "yaml";

const root = resolve(import.meta.dirname, "..");
const connectionTypeDirectory = join(root, "plugins", "connection-types");
const connectionProfileCatalog = join(connectionTypeDirectory, "profiles", "catalog.yaml");
const dialectDirectory = join(root, "plugins", "dialects");
const jsonTarget = join(root, "crates", "chiron-horizon-core", "assets", "database-drivers.manifest.json");
const typeTarget = join(root, "apps", "desktop", "src", "types", "generated", "databaseTypes.ts");
const profileTarget = join(root, "apps", "desktop", "src", "types", "generated", "connectionProfiles.ts");
const checkOnly = process.argv.includes("--check");

const runtimeModes = new Set(["native", "file", "agent", "external"]);
const mcpModes = new Set(["direct", "bridge", "unsupported"]);
const supportLevels = new Set(["connect", "browse", "understand", "operate"]);
const formKinds = new Set(["standard", "jdbc", "mq", "mqtt", "nacos"]);
const connectionPickerModes = new Set(["direct", "merged-profile", "version-selector", "plugin-provided"]);
const runtimeProviders = new Set(["native", "agent", "jdbc", "external-plugin", "specialized-service"]);
const requiredRuntimeFeatures = new Set(["duckdb-sidecar", "dynamodb", "mq-admin"]);
const schemaViewerKinds = new Set(["relational", "document", "graph", "hybrid", "vector", "timeseries", "wide-column", "key-value", "service", "dynamic"]);
const schemaViewerProviders = new Set(["relational-metadata", "document-metadata", "graph-metadata", "hybrid-metadata", "vector-metadata", "timeseries-metadata", "wide-column-metadata", "key-value-metadata", "service-metadata", "runtime-handshake"]);
const schemaViewerScopeLevels = new Set(["catalog", "database", "schema", "object"]);
const profileCategories = new Set(["sql", "analytics", "domestic", "lightweight", "document", "graph_ai", "timeseries", "mq", "registry_config"]);
const descriptorKeys = new Set([
  "schemaVersion",
  "order",
  "dbType",
  "rustVariant",
  "label",
  "dialect",
  "runtimeMode",
  "mcpMode",
  "agentKey",
  "driverStoreVisible",
  "driverStoreOrder",
  "driverProfiles",
  "managedDrivers",
  "singleConnectionPool",
  "metadataConnectionScoped",
  "skipTcpProbe",
  "defaultPort",
  "localFile",
  "specializedSurface",
  "formKind",
  "traits",
  "supportLevel",
  "access",
  "schemaViewer",
  "capabilities",
]);
const profileCatalogKeys = new Set(["schemaVersion", "profiles"]);
const connectionProfileKeys = new Set(["id", "dbType", "label", "pickerLabel", "icon", "pickerIcon", "port", "user", "host", "urlParams", "category"]);
const driverProfileKeys = new Set(["profile", "agentKey", "packageKey", "label", "storeVisible", "storeOrder"]);
const managedDriverKeys = new Set(["key", "label", "storeVisible", "storeOrder"]);
const accessKeys = new Set(["connectionPicker", "runtimeProvider", "connectSupported", "testConnectionSupported", "requiredFeatures"]);
const schemaViewerKeys = new Set(["kind", "provider", "scopeLevels"]);
const capabilityKeys = new Set([
  "queryExecution",
  "metadataBrowse",
  "objectBrowser",
  "objectSource",
  "schemaSearch",
  "diagram",
  "schemaViewer",
  "tableDataEdit",
  "tableStructureEdit",
  "tableImport",
  "dataTransfer",
  "sqlFileExecution",
  "databaseCreate",
  "fieldLineage",
  "sqlExplain",
  "userAdmin",
  "driverManagement",
]);
const traitKeys = new Set(["schemaAware", "databaseSchemaQualified", "singleDatabase", "clearableQuerySchema", "fetchFirst", "treeSchema", "databaseObjectTree", "pgVacuum", "pgLikeStructure", "diagramSql"]);

function connectionTypeFiles() {
  if (!existsSync(connectionTypeDirectory)) throw new Error(`Missing connection type descriptor directory: ${connectionTypeDirectory}`);
  return readdirSync(connectionTypeDirectory)
    .filter((file) => file.endsWith(".yaml") || file.endsWith(".yml"))
    .sort();
}

function dialectNames() {
  return new Set(
    readdirSync(dialectDirectory)
      .filter((file) => file.endsWith(".yaml") || file.endsWith(".yml"))
      .map((file) => {
        const value = parse(readFileSync(join(dialectDirectory, file), "utf8"));
        return value?.dialect?.name;
      })
      .filter(Boolean),
  );
}

function loadDescriptors() {
  const knownDialects = dialectNames();
  const descriptors = connectionTypeFiles().map((file) => {
    const path = join(connectionTypeDirectory, file);
    const descriptor = parse(readFileSync(path, "utf8"));
    validateDescriptor(descriptor, file, knownDialects);
    return descriptor;
  });

  const duplicateDbTypes = duplicateValues(descriptors.map((descriptor) => descriptor.dbType));
  const duplicateRustVariants = duplicateValues(descriptors.map((descriptor) => descriptor.rustVariant));
  const duplicateOrders = duplicateValues(descriptors.map((descriptor) => descriptor.order));
  if (duplicateDbTypes.length > 0) throw new Error(`Duplicate database types: ${duplicateDbTypes.join(", ")}`);
  if (duplicateRustVariants.length > 0) throw new Error(`Duplicate Rust variants: ${duplicateRustVariants.join(", ")}`);
  if (duplicateOrders.length > 0) throw new Error(`Duplicate connection type orders: ${duplicateOrders.join(", ")}`);
  validateDriverStoreOrders(descriptors);

  return descriptors.sort((left, right) => left.order - right.order);
}

function validateDescriptor(descriptor, file, knownDialects) {
  const location = join("plugins", "connection-types", file);
  if (!descriptor || typeof descriptor !== "object") throw new Error(`${location}: descriptor must be a mapping`);
  validateKnownKeys(descriptor, descriptorKeys, location);
  if (descriptor.schemaVersion !== 1) throw new Error(`${location}: schemaVersion must be 1`);
  if (!Number.isInteger(descriptor.order) || descriptor.order <= 0) throw new Error(`${location}: order must be a positive integer`);
  if (!/^[a-z0-9]+(?:-[a-z0-9]+)*$/.test(descriptor.dbType ?? "")) throw new Error(`${location}: invalid dbType`);
  if (!/^[A-Z][A-Za-z0-9]*$/.test(descriptor.rustVariant ?? "")) throw new Error(`${location}: invalid rustVariant`);
  if (typeof descriptor.label !== "string" || descriptor.label.trim() === "") throw new Error(`${location}: label is required`);
  if (!runtimeModes.has(descriptor.runtimeMode)) throw new Error(`${location}: invalid runtimeMode`);
  if (!mcpModes.has(descriptor.mcpMode)) throw new Error(`${location}: invalid mcpMode`);
  if (!supportLevels.has(descriptor.supportLevel)) throw new Error(`${location}: invalid supportLevel`);
  validateAccessContract(descriptor.access, location);
  const providerRuntimeModes = {
    native: new Set(["native", "file"]),
    agent: new Set(["agent"]),
    jdbc: new Set(["external"]),
    "external-plugin": new Set(["external"]),
    "specialized-service": new Set(["native", "agent"]),
  };
  if (!providerRuntimeModes[descriptor.access.runtimeProvider].has(descriptor.runtimeMode)) {
    throw new Error(`${location}: access.runtimeProvider ${descriptor.access.runtimeProvider} is incompatible with runtimeMode ${descriptor.runtimeMode}`);
  }
  if (descriptor.access.runtimeProvider === "agent" && !descriptor.agentKey) {
    throw new Error(`${location}: agent access requires agentKey`);
  }
  validateSchemaViewerContract(descriptor.schemaViewer, location);
  if (descriptor.dialect && !knownDialects.has(descriptor.dialect)) throw new Error(`${location}: unknown dialect ${descriptor.dialect}`);
  if (descriptor.runtimeMode === "agent" && !descriptor.agentKey) throw new Error(`${location}: agent runtime requires agentKey`);
  if (descriptor.driverStoreVisible && !descriptor.agentKey) throw new Error(`${location}: driverStoreVisible requires agentKey`);
  if (descriptor.driverStoreVisible) validatePositiveInteger(descriptor.driverStoreOrder, `${location}: driverStoreOrder`);
  if (descriptor.defaultPort !== undefined && (!Number.isInteger(descriptor.defaultPort) || descriptor.defaultPort < 0 || descriptor.defaultPort > 65535)) {
    throw new Error(`${location}: invalid defaultPort`);
  }
  if (!descriptor.capabilities || typeof descriptor.capabilities !== "object") throw new Error(`${location}: capabilities are required`);
  validateBooleanMap(descriptor.capabilities, capabilityKeys, `${location}: capabilities`);
  const missingCapabilities = [...capabilityKeys].filter((key) => !(key in descriptor.capabilities));
  if (missingCapabilities.length > 0) throw new Error(`${location}: capabilities is missing keys: ${missingCapabilities.join(", ")}`);
  if (descriptor.capabilities.schemaViewer !== true) throw new Error(`${location}: explicit schemaViewer capability must be enabled`);
  if (!Object.values(descriptor.capabilities).some(Boolean) && descriptor.specializedSurface !== true) {
    throw new Error(`${location}: enable a product capability or set specializedSurface: true`);
  }
  if (descriptor.specializedSurface !== undefined && typeof descriptor.specializedSurface !== "boolean") {
    throw new Error(`${location}: specializedSurface must be a boolean`);
  }
  if (descriptor.formKind !== undefined && !formKinds.has(descriptor.formKind)) {
    throw new Error(`${location}: invalid formKind`);
  }
  if (descriptor.traits !== undefined) validateBooleanMap(descriptor.traits, traitKeys, `${location}: traits`);
  for (const profile of descriptor.driverProfiles ?? []) {
    validateKnownKeys(profile, driverProfileKeys, `${location}: driver profile`);
    if (!profile.profile || !profile.agentKey || !profile.label) throw new Error(`${location}: driverProfiles entries require profile, agentKey, and label`);
    if (profile.storeVisible !== undefined && typeof profile.storeVisible !== "boolean") throw new Error(`${location}: driver profile ${profile.profile} storeVisible must be a boolean`);
    if (profile.storeVisible) validatePositiveInteger(profile.storeOrder, `${location}: driver profile ${profile.profile} storeOrder`);
  }
  for (const driver of descriptor.managedDrivers ?? []) {
    validateKnownKeys(driver, managedDriverKeys, `${location}: managed driver`);
    if (!driver.key || !driver.label) throw new Error(`${location}: managedDrivers entries require key and label`);
    if (driver.storeVisible !== undefined && typeof driver.storeVisible !== "boolean") throw new Error(`${location}: managed driver ${driver.key} storeVisible must be a boolean`);
    if (driver.storeVisible) validatePositiveInteger(driver.storeOrder, `${location}: managed driver ${driver.key} storeOrder`);
  }
}

function validateAccessContract(access, location) {
  if (!access || typeof access !== "object" || Array.isArray(access)) throw new Error(`${location}: access contract is required`);
  validateKnownKeys(access, accessKeys, `${location}: access`);
  if (!connectionPickerModes.has(access.connectionPicker)) throw new Error(`${location}: invalid access.connectionPicker`);
  if (!runtimeProviders.has(access.runtimeProvider)) throw new Error(`${location}: invalid access.runtimeProvider`);
  if (access.connectSupported !== true || access.testConnectionSupported !== true) {
    throw new Error(`${location}: all registered connection types must support connect and test connection`);
  }
  if (!Array.isArray(access.requiredFeatures) || access.requiredFeatures.some((feature) => typeof feature !== "string" || !feature.trim())) {
    throw new Error(`${location}: access.requiredFeatures must be a string list`);
  }
  const unknownFeatures = access.requiredFeatures.filter((feature) => !requiredRuntimeFeatures.has(feature));
  if (unknownFeatures.length > 0) throw new Error(`${location}: unknown access.requiredFeatures: ${unknownFeatures.join(", ")}`);
}

function validateSchemaViewerContract(schemaViewer, location) {
  if (!schemaViewer || typeof schemaViewer !== "object" || Array.isArray(schemaViewer)) throw new Error(`${location}: schemaViewer contract is required`);
  validateKnownKeys(schemaViewer, schemaViewerKeys, `${location}: schemaViewer`);
  if (!schemaViewerKinds.has(schemaViewer.kind)) throw new Error(`${location}: invalid schemaViewer.kind`);
  if (!schemaViewerProviders.has(schemaViewer.provider)) throw new Error(`${location}: invalid schemaViewer.provider`);
  if (!Array.isArray(schemaViewer.scopeLevels) || schemaViewer.scopeLevels.length === 0 || schemaViewer.scopeLevels.some((level) => !schemaViewerScopeLevels.has(level))) {
    throw new Error(`${location}: invalid schemaViewer.scopeLevels`);
  }
}

function loadConnectionProfiles(descriptors) {
  if (!existsSync(connectionProfileCatalog)) throw new Error(`Missing connection profile catalog: ${connectionProfileCatalog}`);
  const location = join("plugins", "connection-types", "profiles", "catalog.yaml");
  const catalog = parse(readFileSync(connectionProfileCatalog, "utf8"));
  if (!catalog || typeof catalog !== "object") throw new Error(`${location}: catalog must be a mapping`);
  validateKnownKeys(catalog, profileCatalogKeys, location);
  if (catalog.schemaVersion !== 1) throw new Error(`${location}: schemaVersion must be 1`);
  if (!Array.isArray(catalog.profiles) || catalog.profiles.length === 0) throw new Error(`${location}: profiles must be a non-empty list`);

  const knownTypes = new Set(descriptors.map((descriptor) => descriptor.dbType));
  const profileIds = new Set();
  const profiles = catalog.profiles.map((profile, index) => {
    const profileLocation = `${location}: profiles[${index}]`;
    if (!profile || typeof profile !== "object") throw new Error(`${profileLocation} must be a mapping`);
    validateKnownKeys(profile, connectionProfileKeys, profileLocation);
    if (!/^[a-z0-9]+(?:[-_][a-z0-9]+)*$/.test(profile.id ?? "")) throw new Error(`${profileLocation}: invalid id`);
    if (profileIds.has(profile.id)) throw new Error(`${profileLocation}: duplicate id ${profile.id}`);
    profileIds.add(profile.id);
    if (!knownTypes.has(profile.dbType)) throw new Error(`${profileLocation}: unknown dbType ${profile.dbType}`);
    for (const key of ["label", "icon", "user"]) {
      if (typeof profile[key] !== "string") throw new Error(`${profileLocation}: ${key} must be a string`);
    }
    for (const key of ["pickerLabel", "pickerIcon", "host", "urlParams"]) {
      if (profile[key] !== undefined && typeof profile[key] !== "string") throw new Error(`${profileLocation}: ${key} must be a string`);
    }
    if (!Number.isInteger(profile.port) || profile.port < 0 || profile.port > 65535) throw new Error(`${profileLocation}: invalid port`);
    if (profile.category !== undefined && !profileCategories.has(profile.category)) throw new Error(`${profileLocation}: invalid category`);
    return profile;
  });

  validateAccessProfileCoverage(descriptors, profiles);

  return profiles;
}

function validateAccessProfileCoverage(descriptors, profiles) {
  const profilesByType = new Map();
  for (const profile of profiles) {
    const entries = profilesByType.get(profile.dbType) ?? [];
    entries.push(profile);
    profilesByType.set(profile.dbType, entries);
  }
  for (const descriptor of descriptors) {
    const pickerMode = descriptor.access.connectionPicker;
    const typeProfiles = profilesByType.get(descriptor.dbType) ?? [];
    if (pickerMode === "plugin-provided") {
      if (descriptor.dbType !== "plugin" || descriptor.access.runtimeProvider !== "external-plugin") {
        throw new Error(`${descriptor.dbType}: plugin-provided picker requires the external-plugin runtime`);
      }
      continue;
    }
    if (typeProfiles.length === 0) {
      throw new Error(`${descriptor.dbType}: ${pickerMode} access requires at least one connection profile`);
    }
    if (pickerMode === "direct" && !typeProfiles.some((profile) => profile.category)) {
      throw new Error(`${descriptor.dbType}: no connection profile is reachable from the picker`);
    }
  }
}

function validateDriverStoreOrders(descriptors) {
  const entries = descriptors.flatMap((descriptor) => [
    ...(descriptor.driverStoreVisible ? [{ key: descriptor.agentKey, order: descriptor.driverStoreOrder }] : []),
    ...(descriptor.driverProfiles ?? []).filter((profile) => profile.storeVisible).map((profile) => ({ key: profile.packageKey ?? profile.agentKey, order: profile.storeOrder })),
    ...(descriptor.managedDrivers ?? []).filter((driver) => driver.storeVisible).map((driver) => ({ key: driver.key, order: driver.storeOrder })),
  ]);
  const orderByKey = new Map();
  for (const entry of entries) {
    const existingOrder = orderByKey.get(entry.key);
    if (existingOrder !== undefined && existingOrder !== entry.order) {
      throw new Error(`Driver store key ${entry.key} uses conflicting orders: ${existingOrder}, ${entry.order}`);
    }
    orderByKey.set(entry.key, entry.order);
  }
  const duplicateStoreOrders = duplicateValues([...orderByKey.values()]);
  if (duplicateStoreOrders.length > 0) throw new Error(`Duplicate driver store orders: ${duplicateStoreOrders.join(", ")}`);
}

function validatePositiveInteger(value, location) {
  if (!Number.isInteger(value) || value <= 0) throw new Error(`${location} must be a positive integer`);
}

function validateKnownKeys(value, allowedKeys, location) {
  const unknownKeys = Object.keys(value).filter((key) => !allowedKeys.has(key));
  if (unknownKeys.length > 0) throw new Error(`${location} contains unknown keys: ${unknownKeys.join(", ")}`);
}

function validateBooleanMap(value, allowedKeys, location, requireAll = false) {
  if (!value || typeof value !== "object" || Array.isArray(value)) throw new Error(`${location} must be a mapping`);
  validateKnownKeys(value, allowedKeys, location);
  if (requireAll) {
    const missingKeys = [...allowedKeys].filter((key) => !(key in value));
    if (missingKeys.length > 0) throw new Error(`${location} is missing keys: ${missingKeys.join(", ")}`);
  }
  const nonBooleanKeys = Object.entries(value)
    .filter(([, entry]) => typeof entry !== "boolean")
    .map(([key]) => key);
  if (nonBooleanKeys.length > 0) throw new Error(`${location} must use boolean values: ${nonBooleanKeys.join(", ")}`);
}

function duplicateValues(values) {
  return [...new Set(values.filter((value, index) => values.indexOf(value) !== index))];
}

function publicDescriptor(descriptor) {
  const { schemaVersion: _schemaVersion, order: _order, rustVariant: _rustVariant, ...publicFields } = descriptor;
  return publicFields;
}

function jsonOutput(descriptors) {
  return `${JSON.stringify({ schemaVersion: 1, drivers: descriptors.map(publicDescriptor) }, null, 2)}\n`;
}

function typeOutput(descriptors) {
  const values = descriptors.map((descriptor) => `  "${descriptor.dbType}",`).join("\n");
  return `// Generated by scripts/sync-connection-types.mjs. Do not edit manually.\nexport const DATABASE_TYPES = [\n${values}\n] as const;\n\nexport type DatabaseType = (typeof DATABASE_TYPES)[number];\n`;
}

function typescriptPropertyName(value) {
  return /^[A-Za-z_$][A-Za-z0-9_$]*$/.test(value) ? value : JSON.stringify(value);
}

function profileTypeOutput(profiles) {
  const profileEntries = profiles
    .map((profile) => {
      const fields = [
        `type: ${JSON.stringify(profile.dbType)}`,
        `port: ${profile.port}`,
        `user: ${JSON.stringify(profile.user)}`,
        `label: ${JSON.stringify(profile.label)}`,
        `icon: ${JSON.stringify(profile.icon)}`,
        ...(profile.host === undefined ? [] : [`host: ${JSON.stringify(profile.host)}`]),
        ...(profile.urlParams === undefined ? [] : [`urlParams: ${JSON.stringify(profile.urlParams)}`]),
      ];
      return `  ${typescriptPropertyName(profile.id)}: { ${fields.join(", ")} },`;
    })
    .join("\n");
  const pickerOptions = profiles
    .filter((profile) => profile.category)
    .map((profile) => `  { value: ${JSON.stringify(profile.id)}, label: ${JSON.stringify(profile.pickerLabel ?? profile.label)}, category: ${JSON.stringify(profile.category)} },`)
    .join("\n");
  const profileIcons = profiles.map((profile) => `  ${typescriptPropertyName(profile.id)}: ${JSON.stringify(profile.pickerIcon ?? profile.icon)},`).join("\n");
  return `// Generated by scripts/sync-connection-types.mjs. Do not edit manually.\nimport type { DatabaseType } from "./databaseTypes";\n\nexport type ConnectionProfileCategory = "sql" | "analytics" | "domestic" | "lightweight" | "document" | "graph_ai" | "timeseries" | "mq" | "registry_config";\n\nexport interface ConnectionProfileDefinition {\n  type: DatabaseType;\n  port: number;\n  user: string;\n  label: string;\n  icon: string;\n  host?: string;\n  urlParams?: string;\n}\n\nexport interface ConnectionPickerOption {\n  value: string;\n  label: string;\n  category: ConnectionProfileCategory;\n}\n\nexport const CONNECTION_PROFILES = {\n${profileEntries}\n} as const satisfies Record<string, ConnectionProfileDefinition>;\n\nexport const CONNECTION_PROFILE_ICONS = {\n${profileIcons}\n} as const satisfies Record<string, string>;\n\nexport const CONNECTION_PICKER_OPTIONS = [\n${pickerOptions}\n] as const satisfies readonly ConnectionPickerOption[];\n`;
}

function syncTarget(path, expected) {
  const current = existsSync(path) ? readFileSync(path, "utf8") : "";
  if (current === expected) return;
  if (checkOnly) throw new Error(`${path.slice(root.length + 1)} is out of date; run pnpm generate:connection-types`);
  mkdirSync(dirname(path), { recursive: true });
  writeFileSync(path, expected);
}

try {
  const descriptors = loadDescriptors();
  const profiles = loadConnectionProfiles(descriptors);
  syncTarget(jsonTarget, jsonOutput(descriptors));
  syncTarget(typeTarget, typeOutput(descriptors));
  syncTarget(profileTarget, profileTypeOutput(profiles));
  console.log(`${checkOnly ? "Validated" : "Generated"} ${descriptors.length} connection type descriptors and ${profiles.length} connection profiles.`);
} catch (error) {
  console.error(error instanceof Error ? error.message : String(error));
  process.exit(1);
}
