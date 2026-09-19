// @vitest-environment happy-dom

import { createApp, h, nextTick } from "vue";
import { afterEach, describe, expect, it } from "vitest";
import SchemaCompositeView from "../SchemaCompositeView.vue";
import SchemaGraphView from "../SchemaGraphView.vue";
import SchemaJsonTree from "../SchemaJsonTree.vue";

const mounted: Array<ReturnType<typeof createApp>> = [];

function mount(component: Parameters<typeof h>[0], props: Record<string, unknown>) {
  const host = document.createElement("div");
  document.body.append(host);
  const app = createApp({ render: () => h(component, props) });
  mounted.push(app);
  app.mount(host);
  return host;
}

afterEach(() => {
  for (const app of mounted.splice(0)) app.unmount();
  document.body.innerHTML = "";
});

describe("Schema Viewer renderers", () => {
  it("renders graph labels and relationship types without querying graph data", () => {
    const host = mount(SchemaGraphView, {
      nodes: [
        { id: "person", label: "Person", properties: ["email"] },
        { id: "company", label: "Company", properties: [] },
      ],
      edges: [{ type: "WORKS_AT", source: "person", target: "company" }],
    });
    expect(host.textContent).toContain("Person");
    expect(host.textContent).toContain("WORKS_AT");
    expect(host.querySelector('svg[aria-label="Graph schema metagraph"]')).not.toBeNull();
    expect(host.querySelector("svg line")).not.toBeNull();
  });

  it("switches composite facets independently", async () => {
    const host = mount(SchemaCompositeView, { value: { document: { validator: true }, vector: { dimensions: 1536 } } });
    expect(host.textContent).toContain("validator");
    const vector = [...host.querySelectorAll("button")].find((button) => button.textContent === "vector")!;
    vector.click();
    await nextTick();
    expect(host.textContent).toContain("1536");
  });

  it("keeps declared document metadata expandable", async () => {
    const host = mount(SchemaJsonTree, { value: { properties: { id: { bsonType: "string" } } } });
    expect(host.textContent).toContain("properties");
    const button = host.querySelector("button")!;
    button.click();
    await nextTick();
    expect(host.textContent).not.toContain("bsonType");
  });
});
