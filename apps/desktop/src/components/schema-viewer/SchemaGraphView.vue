<script setup lang="ts">
import { computed } from "vue";

type GraphNode = { id?: string; label?: string; properties?: unknown[] };
type GraphEdge = { type?: string; source?: string | null; target?: string | null };

const props = defineProps<{ nodes: GraphNode[]; edges: GraphEdge[] }>();
const width = 820;
const height = 440;
const positionedNodes = computed(() => {
  const count = Math.max(props.nodes.length, 1);
  const radius = Math.min(width, height) * 0.34;
  return props.nodes.map((node, index) => {
    const angle = (index / count) * Math.PI * 2 - Math.PI / 2;
    return { ...node, x: width / 2 + Math.cos(angle) * radius, y: height / 2 + Math.sin(angle) * radius };
  });
});
const nodeById = computed(() => new Map(positionedNodes.value.map((node) => [node.id || node.label || "", node])));
const drawableEdges = computed(() =>
  props.edges.flatMap((edge) => {
    const source = edge.source ? nodeById.value.get(edge.source) : undefined;
    const target = edge.target ? nodeById.value.get(edge.target) : undefined;
    return source && target ? [{ ...edge, sourceNode: source, targetNode: target }] : [];
  }),
);
</script>

<template>
  <div class="grid min-h-0 gap-3 lg:grid-cols-[minmax(0,1fr)_14rem]">
    <svg class="h-[28rem] w-full rounded-md border bg-muted/10" :viewBox="`0 0 ${width} ${height}`" role="img" aria-label="Graph schema metagraph">
      <g v-for="edge in drawableEdges" :key="`${edge.source}-${edge.type}-${edge.target}`">
        <line :x1="edge.sourceNode.x" :y1="edge.sourceNode.y" :x2="edge.targetNode.x" :y2="edge.targetNode.y" class="stroke-muted-foreground/50" stroke-width="2" />
        <text :x="(edge.sourceNode.x + edge.targetNode.x) / 2" :y="(edge.sourceNode.y + edge.targetNode.y) / 2 - 6" text-anchor="middle" class="fill-muted-foreground text-[11px]">{{ edge.type }}</text>
      </g>
      <g v-for="node in positionedNodes" :key="node.id || node.label" :transform="`translate(${node.x},${node.y})`">
        <rect x="-72" y="-28" width="144" height="56" rx="9" class="fill-background stroke-border" stroke-width="2" />
        <text text-anchor="middle" y="-2" class="fill-foreground text-[13px] font-medium">{{ node.label || node.id }}</text>
        <text text-anchor="middle" y="15" class="fill-muted-foreground text-[10px]">{{ node.properties?.length || 0 }} properties</text>
      </g>
    </svg>
    <aside class="min-h-0 overflow-auto rounded-md border p-3">
      <p class="mb-2 text-xs font-semibold uppercase tracking-wide text-muted-foreground">Relationship types</p>
      <p v-if="!edges.length" class="text-xs text-muted-foreground">No relationship types exposed.</p>
      <div v-for="(edge, index) in edges" :key="`${edge.type}-${index}`" class="mb-2 rounded border p-2 text-xs">
        <p class="font-medium">{{ edge.type || "Relationship" }}</p>
        <p class="text-muted-foreground">{{ edge.source || "any label" }} → {{ edge.target || "any label" }}</p>
      </div>
    </aside>
  </div>
</template>
