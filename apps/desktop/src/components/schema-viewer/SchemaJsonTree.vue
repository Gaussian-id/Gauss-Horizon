<script setup lang="ts">
import { computed, ref } from "vue";
import { ChevronDown, ChevronRight } from "@lucide/vue";

const props = defineProps<{ value: unknown; name?: string; depth?: number }>();
const open = ref((props.depth ?? 0) < 2);
const isObject = computed(() => typeof props.value === "object" && props.value !== null);
const entries = computed(() => {
  if (Array.isArray(props.value)) return props.value.map((value, index) => [String(index), value] as const);
  if (isObject.value) return Object.entries(props.value as Record<string, unknown>);
  return [];
});
const preview = computed(() => {
  if (props.value === null) return "null";
  if (typeof props.value === "string") return JSON.stringify(props.value);
  return String(props.value);
});
const collectionPreview = computed(() => (Array.isArray(props.value) ? `[${entries.value.length}]` : `{${entries.value.length}}`));
</script>

<template>
  <div class="font-mono text-xs leading-5">
    <button v-if="isObject" type="button" class="flex items-center gap-1 text-left hover:text-primary" @click="open = !open">
      <ChevronDown v-if="open" class="h-3 w-3" /><ChevronRight v-else class="h-3 w-3" />
      <span v-if="name" class="text-foreground">{{ name }}:</span>
      <span class="text-muted-foreground">{{ collectionPreview }}</span>
    </button>
    <div v-else class="flex gap-1">
      <span v-if="name" class="text-foreground">{{ name }}:</span><span class="text-muted-foreground break-all">{{ preview }}</span>
    </div>
    <div v-if="isObject && open" class="ml-4 border-l border-border pl-2">
      <SchemaJsonTree v-for="[key, entry] in entries" :key="key" :name="key" :value="entry" :depth="(depth ?? 0) + 1" />
    </div>
  </div>
</template>
