<script setup lang="ts">
import { computed, ref, watch } from "vue";
import SchemaJsonTree from "./SchemaJsonTree.vue";

const props = defineProps<{ value: unknown }>();
const entries = computed(() => {
  if (!props.value || typeof props.value !== "object" || Array.isArray(props.value)) return [["Metadata", props.value]] as Array<[string, unknown]>;
  return Object.entries(props.value as Record<string, unknown>);
});
const active = ref("");
watch(
  entries,
  (value) => {
    if (!value.some(([key]) => key === active.value)) active.value = value[0]?.[0] ?? "";
  },
  { immediate: true },
);
const activeValue = computed(() => entries.value.find(([key]) => key === active.value)?.[1]);
</script>

<template>
  <div class="min-h-0 overflow-hidden rounded-md border">
    <div class="flex gap-1 overflow-x-auto border-b bg-muted/20 p-2">
      <button v-for="[key] in entries" :key="key" type="button" class="rounded px-3 py-1.5 text-xs font-medium" :class="active === key ? 'bg-background shadow-sm' : 'text-muted-foreground hover:text-foreground'" @click="active = key">{{ key }}</button>
    </div>
    <div class="max-h-[32rem] overflow-auto p-4"><SchemaJsonTree :value="activeValue" /></div>
  </div>
</template>
