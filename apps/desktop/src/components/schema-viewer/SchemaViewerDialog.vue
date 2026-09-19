<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { Loader2, Network, RefreshCw, Table2 } from "@lucide/vue";
import { Dialog, DialogContent, DialogDescription, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { useConnectionStore } from "@/stores/connectionStore";
import * as api from "@/lib/backend/api";
import { supportsSchemaViewer } from "@/lib/database/databaseFeatureSupport";
import { schemaViewerKindLabel, schemaViewerScopeLabel, type SchemaScopePage, type SchemaViewResponse, type SchemaViewScope, type SchemaViewerDescriptor } from "@/lib/database/schemaViewer";
import SchemaJsonTree from "./SchemaJsonTree.vue";
import SchemaGraphView from "./SchemaGraphView.vue";
import SchemaCompositeView from "./SchemaCompositeView.vue";

const open = defineModel<boolean>("open", { default: false });
const store = useConnectionStore();
const { t } = useI18n();

const connectionId = ref("");
const descriptor = ref<SchemaViewerDescriptor>();
const catalog = ref("");
const database = ref("");
const schema = ref("");
const object = ref("");
const databaseScopes = ref<SchemaViewScope[]>([]);
const catalogScopes = ref<SchemaViewScope[]>([]);
const schemaScopes = ref<SchemaViewScope[]>([]);
const objectScopes = ref<SchemaViewScope[]>([]);
const objectScopeSearch = ref("");
const objectNextOffset = ref<number | null>(null);
const loadingScopes = ref(false);
const loading = ref(false);
const error = ref("");
const response = ref<SchemaViewResponse>();
const requestedScope = ref<SchemaViewScope>();
const metadataSearch = ref("");
const supportedConnectionIds = ref<Set<string>>(new Set());

const candidateConnections = computed(() => store.connections.filter((connection) => store.connectedIds.has(connection.id) && supportsSchemaViewer(connection.db_type)));
const connections = computed(() => candidateConnections.value.filter((connection) => supportedConnectionIds.value.has(connection.id)));
const connection = computed(() => connections.value.find((item) => item.id === connectionId.value));
const title = computed(() => (descriptor.value ? schemaViewerKindLabel(descriptor.value.kind) : t("schemaViewer.title")));
const needsSchema = computed(() => descriptor.value?.scopeLevels.includes("schema") === true);
const needsCatalog = computed(() => descriptor.value?.scopeLevels.includes("catalog") === true);
const needsObject = computed(() => descriptor.value?.scopeLevels.includes("object") === true);
const payloadData = computed(() => response.value?.payload.data ?? null);
const tableRows = computed(() => {
  const data = payloadData.value as { tables?: Array<{ table: { name: string; table_type?: string }; columns: unknown[]; indexes: unknown[]; foreignKeys: unknown[] }> } | null;
  const rows = data?.tables ?? [];
  const needle = metadataSearch.value.trim().toLowerCase();
  return needle ? rows.filter((row) => row.table.name.toLowerCase().includes(needle)) : rows;
});
const graphNodes = computed(() => {
  const data = payloadData.value as { nodes?: Array<{ id?: string; label?: string; properties?: unknown[] }> } | null;
  return data?.nodes ?? [];
});
const graphEdges = computed(() => {
  const data = payloadData.value as { edges?: Array<{ type?: string; source?: string | null; target?: string | null }> } | null;
  return data?.edges ?? [];
});
const graphMetadata = computed(() => {
  const data = payloadData.value as { metadata?: unknown } | null;
  return data?.metadata;
});
const filteredPayloadData = computed(() => filterMetadataValue(payloadData.value, metadataSearch.value.trim().toLowerCase()));

function filterMetadataValue(value: unknown, needle: string): unknown {
  if (!needle) return value;
  if (Array.isArray(value)) {
    const matches = value.map((item) => filterMetadataValue(item, needle)).filter((item) => item !== undefined);
    return matches.length ? matches : undefined;
  }
  if (value && typeof value === "object") {
    const matches = Object.entries(value).flatMap(([key, child]) => {
      if (key.toLowerCase().includes(needle)) return [[key, child] as const];
      const filtered = filterMetadataValue(child, needle);
      return filtered === undefined ? [] : [[key, filtered] as const];
    });
    return matches.length ? Object.fromEntries(matches) : undefined;
  }
  return String(value ?? "")
    .toLowerCase()
    .includes(needle)
    ? value
    : undefined;
}

function pageItems(page: SchemaScopePage): SchemaViewScope[] {
  return page.items ?? [];
}

function selectedScope(): SchemaViewScope {
  return {
    catalog: catalog.value || undefined,
    database: database.value || undefined,
    schema: schema.value || undefined,
    object: object.value || undefined,
    objectKind: objectScopes.value.find((item) => item.object === object.value)?.objectKind,
  };
}

async function loadConnectionContract() {
  const current = connection.value;
  if (!current) return;
  loadingScopes.value = true;
  error.value = "";
  response.value = undefined;
  databaseScopes.value = [];
  catalogScopes.value = [];
  schemaScopes.value = [];
  objectScopes.value = [];
  objectNextOffset.value = null;
  try {
    descriptor.value = await api.describeSchemaViewer(current.id);
    const page = await api.listSchemaViewerScopes(current.id, {});
    if (needsCatalog.value) {
      catalogScopes.value = pageItems(page);
      catalog.value = catalogScopes.value.find((item) => item.catalog === requestedScope.value?.catalog)?.catalog ?? requestedScope.value?.catalog ?? catalogScopes.value[0]?.catalog ?? "";
      databaseScopes.value = pageItems(await api.listSchemaViewerScopes(current.id, { catalog: catalog.value || undefined }));
    } else {
      catalog.value = "";
      databaseScopes.value = pageItems(page);
    }
    const preferredDatabase = requestedScope.value?.database || current.database;
    database.value = databaseScopes.value.find((item) => item.database === preferredDatabase)?.database ?? preferredDatabase ?? databaseScopes.value[0]?.database ?? "";
    await loadChildScopes(requestedScope.value?.schema, requestedScope.value?.object);
    requestedScope.value = undefined;
  } catch (cause) {
    error.value = cause instanceof Error ? cause.message : String(cause);
  } finally {
    loadingScopes.value = false;
  }
}

async function loadChildScopes(preferredSchema?: string, preferredObject?: string) {
  const current = connection.value;
  if (!current || !descriptor.value) return;
  schemaScopes.value = [];
  objectScopes.value = [];
  schema.value = "";
  object.value = "";
  if (needsSchema.value && database.value) {
    const page = await api.listSchemaViewerScopes(current.id, { database: database.value });
    schemaScopes.value = pageItems(page);
    schema.value = schemaScopes.value.find((item) => item.schema === preferredSchema)?.schema ?? preferredSchema ?? schemaScopes.value[0]?.schema ?? "";
  }
  if (needsObject.value && database.value) await loadObjectScopes(preferredObject);
  else if (preferredObject) object.value = preferredObject;
}

async function loadCatalogChildren() {
  const current = connection.value;
  if (!current || !needsCatalog.value) return;
  databaseScopes.value = pageItems(await api.listSchemaViewerScopes(current.id, { catalog: catalog.value || undefined }));
  database.value = databaseScopes.value[0]?.database ?? "";
  await loadChildScopes();
}

async function loadObjectScopes(preferredObject?: string, append = false) {
  const current = connection.value;
  if (!current || !database.value) return;
  const offset = append ? (objectNextOffset.value ?? 0) : 0;
  const page = await api.listSchemaViewerScopes(current.id, { database: database.value, schema: schema.value || undefined }, objectScopeSearch.value.trim() || undefined, 100, offset);
  objectScopes.value = append ? [...objectScopes.value, ...pageItems(page)] : pageItems(page);
  objectNextOffset.value = page.nextOffset ?? null;
  object.value = objectScopes.value.find((item) => item.object === preferredObject)?.object ?? preferredObject ?? objectScopes.value[0]?.object ?? "";
}

async function searchObjectScopes() {
  await loadObjectScopes();
}

async function loadMoreObjectScopes() {
  if (objectNextOffset.value === null) return;
  await loadObjectScopes(object.value, true);
}

async function loadMetadata() {
  const current = connection.value;
  if (!current || !descriptor.value) return;
  loading.value = true;
  error.value = "";
  try {
    response.value = await api.getSchemaView(current.id, selectedScope());
  } catch (cause) {
    error.value = cause instanceof Error ? cause.message : String(cause);
  } finally {
    loading.value = false;
  }
}

function openRelationalErd() {
  const current = connection.value;
  if (!current) return;
  store.diagramSource = {
    connectionId: current.id,
    database: database.value || current.database || "default",
    schema: schema.value || undefined,
    tableName: object.value || undefined,
  };
  open.value = false;
}

async function refreshScopes() {
  await loadConnectionContract();
}

async function initializeViewer() {
  loadingScopes.value = true;
  error.value = "";
  const checks = await Promise.all(
    candidateConnections.value.map((item) =>
      api
        .describeSchemaViewer(item.id)
        .then((descriptor) => ({ item, descriptor }))
        .catch(() => null),
    ),
  );
  const supported = checks.filter((check): check is NonNullable<typeof check> => Boolean(check?.descriptor.supported));
  supportedConnectionIds.value = new Set(supported.map((check) => check.item.id));
  const source = store.schemaViewerSource;
  if (source) {
    requestedScope.value = { database: source.database, schema: source.schema, object: source.object };
    connectionId.value = supportedConnectionIds.value.has(source.connectionId) ? source.connectionId : "";
    store.schemaViewerSource = null;
  }
  if (!connectionId.value || !supportedConnectionIds.value.has(connectionId.value)) connectionId.value = connections.value[0]?.id ?? "";
  loadingScopes.value = false;
  if (connectionId.value) await loadConnectionContract();
}

watch(open, (visible) => {
  if (!visible) return;
  void initializeViewer();
});
watch(connectionId, () => {
  if (open.value) void loadConnectionContract();
});
watch(catalog, (value, previous) => {
  if (open.value && value !== previous && !loadingScopes.value) void loadCatalogChildren();
});
watch(database, (value, previous) => {
  if (open.value && value !== previous && !loadingScopes.value) void loadChildScopes();
});
watch(schema, (value, previous) => {
  if (open.value && value !== previous && needsObject.value && !loadingScopes.value) void loadObjectScopes();
});
</script>

<template>
  <Dialog v-model:open="open">
    <DialogContent class="flex max-h-[88vh] max-w-5xl flex-col overflow-hidden">
      <DialogHeader>
        <DialogTitle class="flex items-center gap-2"><Network class="h-4 w-4" />{{ title }}</DialogTitle>
        <DialogDescription>{{ t("schemaViewer.description") }}</DialogDescription>
      </DialogHeader>

      <div class="grid gap-3 rounded-md border bg-muted/20 p-3 sm:grid-cols-4">
        <label class="grid gap-1 text-xs font-medium"
          ><span>{{ t("schemaViewer.connection") }}</span>
          <select v-model="connectionId" class="h-8 rounded-md border bg-background px-2 text-sm">
            <option v-for="item in connections" :key="item.id" :value="item.id">{{ item.name }}</option>
          </select>
        </label>
        <label v-if="needsCatalog" class="grid gap-1 text-xs font-medium"
          ><span>Catalog</span>
          <select v-if="catalogScopes.length" v-model="catalog" class="h-8 rounded-md border bg-background px-2 text-sm">
            <option v-for="item in catalogScopes" :key="item.catalog" :value="item.catalog">{{ item.catalog }}</option>
          </select>
          <Input v-else v-model="catalog" class="h-8" />
        </label>
        <label class="grid gap-1 text-xs font-medium"
          ><span>Database</span>
          <select v-if="databaseScopes.length" v-model="database" class="h-8 rounded-md border bg-background px-2 text-sm">
            <option v-for="item in databaseScopes" :key="item.database" :value="item.database">{{ item.database }}</option>
          </select>
          <Input v-else v-model="database" class="h-8" />
        </label>
        <label v-if="needsSchema" class="grid gap-1 text-xs font-medium"
          ><span>Schema</span>
          <select v-if="schemaScopes.length" v-model="schema" class="h-8 rounded-md border bg-background px-2 text-sm">
            <option v-for="item in schemaScopes" :key="item.schema" :value="item.schema">{{ item.schema }}</option>
          </select>
          <Input v-else v-model="schema" class="h-8" />
        </label>
        <label v-if="needsObject" class="grid gap-1 text-xs font-medium"
          ><span>{{ descriptor ? schemaViewerScopeLabel(descriptor.kind) : t("schemaViewer.scope") }}</span>
          <select v-if="objectScopes.length" v-model="object" class="h-8 rounded-md border bg-background px-2 text-sm">
            <option v-for="item in objectScopes" :key="item.object" :value="item.object">{{ item.object }}</option>
          </select>
          <Input v-else v-model="object" class="h-8" :placeholder="t('schemaViewer.optionalScope')" />
        </label>
        <div v-if="needsObject" class="flex items-end gap-2 sm:col-span-4">
          <Input v-model="objectScopeSearch" class="h-8 max-w-sm" placeholder="Search scopes" @keyup.enter="searchObjectScopes" />
          <Button size="sm" variant="outline" :disabled="loadingScopes" @click="searchObjectScopes">Search</Button>
          <Button v-if="objectNextOffset !== null" size="sm" variant="outline" :disabled="loadingScopes" @click="loadMoreObjectScopes">Load more</Button>
        </div>
        <div class="flex items-center gap-2 sm:col-span-4">
          <Button size="sm" :disabled="!connection || loading || loadingScopes" @click="loadMetadata"><Loader2 v-if="loading" class="mr-1 h-3.5 w-3.5 animate-spin" /><Table2 v-else class="mr-1 h-3.5 w-3.5" />{{ t("schemaViewer.load") }}</Button>
          <Button v-if="descriptor?.kind === 'relational'" size="sm" variant="outline" @click="openRelationalErd">{{ t("schemaViewer.openErd") }}</Button>
          <Button size="sm" variant="outline" :disabled="!connection || loadingScopes" @click="refreshScopes"><RefreshCw class="mr-1 h-3.5 w-3.5" />{{ t("schemaViewer.refresh") }}</Button>
          <span v-if="loadingScopes" class="text-xs text-muted-foreground">{{ t("schemaViewer.loadingScopes") }}</span>
        </div>
      </div>

      <p v-if="!connections.length" class="rounded-md border border-dashed p-6 text-center text-sm text-muted-foreground">{{ t("schemaViewer.noConnections") }}</p>
      <p v-else-if="error" class="rounded-md border border-destructive/40 bg-destructive/5 p-3 text-sm text-destructive">{{ error }}</p>
      <template v-else-if="response">
        <div v-if="response.notices.length || response.availability !== 'complete'" class="rounded-md border border-amber-500/30 bg-amber-500/10 p-3 text-xs text-muted-foreground">
          <p class="font-medium text-foreground">Metadata: {{ response.availability }}</p>
          <p v-for="notice in response.notices" :key="notice">{{ notice }}</p>
          <p v-for="facet in response.facets.filter((item) => item.availability !== 'complete')" :key="facet.name">
            <span class="font-medium">{{ facet.name }}:</span> {{ facet.availability }}<span v-if="facet.error"> — {{ facet.error }}</span>
          </p>
        </div>
        <div v-if="response.kind === 'graph'" class="min-h-0 space-y-3 overflow-auto">
          <SchemaGraphView :nodes="graphNodes" :edges="graphEdges" />
          <details v-if="graphMetadata" class="rounded-md border p-3">
            <summary class="cursor-pointer text-sm font-medium">Native graph metadata</summary>
            <div class="mt-3"><SchemaJsonTree :value="graphMetadata" /></div>
          </details>
        </div>
        <SchemaCompositeView v-else-if="response.kind === 'hybrid'" :value="payloadData" />
        <section v-else-if="tableRows.length" class="min-h-0 overflow-auto rounded-md border">
          <div class="border-b p-2"><Input v-model="metadataSearch" class="h-8" placeholder="Search schema objects" /></div>
          <table class="w-full text-left text-sm">
            <thead class="sticky top-0 bg-muted/80 text-xs text-muted-foreground">
              <tr>
                <th class="px-3 py-2">Object</th>
                <th class="px-3 py-2">Columns</th>
                <th class="px-3 py-2">Indexes</th>
                <th class="px-3 py-2">Foreign keys</th>
              </tr>
            </thead>
            <tbody>
              <tr v-for="row in tableRows" :key="row.table.name" class="border-t">
                <td class="px-3 py-2 font-medium">{{ row.table.name }}</td>
                <td class="px-3 py-2">{{ row.columns.length }}</td>
                <td class="px-3 py-2">{{ row.indexes.length }}</td>
                <td class="px-3 py-2">{{ row.foreignKeys.length }}</td>
              </tr>
            </tbody>
          </table>
        </section>
        <section v-else class="min-h-0 overflow-auto rounded-md border">
          <div class="sticky top-0 z-10 border-b bg-background p-2"><Input v-model="metadataSearch" class="h-8" placeholder="Search native metadata" /></div>
          <div class="p-4"><SchemaJsonTree :value="filteredPayloadData ?? {}" /></div>
        </section>
      </template>
    </DialogContent>
  </Dialog>
</template>
