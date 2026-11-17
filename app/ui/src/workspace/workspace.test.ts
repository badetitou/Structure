import { describe, expect, it, vi } from "vitest";
import type { DockviewApi } from "dockview";

vi.mock("$lib/components/DetailsPanel.svelte", () => ({ default: {} }));
vi.mock("$lib/components/DiffPanel.svelte", () => ({ default: {} }));
vi.mock("$lib/components/HistorySidebar.svelte", () => ({ default: {} }));
vi.mock("$lib/components/MatrixPanel.svelte", () => ({ default: {} }));
vi.mock("../stores/shown", () => ({ shown: { matrix: {} } }));
vi.mock("../stores/git.svelte", () => ({ git: { historyAvailable: true, diffAvailable: true } }));

import { workspace } from "./workspace.svelte";

/** A dockview stand-in: panels by id, `addPanel` / `close` recorded. */
function fakeDock(open: [string, string][]) {
  const panels = new Map<string, { id: string; title: string; api: { component: string; close(): void; setActive(): void } }>();
  const added: Record<string, unknown>[] = [];
  const add = (id: string, component: string, title = id) =>
    panels.set(id, { id, title, api: { component, close: () => void panels.delete(id), setActive: () => {} } });
  for (const [id, c] of open) add(id, c);
  const api = {
    get panels() {
      return [...panels.values()];
    },
    getPanel: (id: string) => panels.get(id),
    addPanel(o: { id: string; component: string; title: string }) {
      added.push(o);
      add(o.id, o.component, o.title);
    },
  };
  return { api: api as unknown as DockviewApi, added };
}

describe("workspace", () => {
  it("opens one more instance of a multi-instance panel beside the active one", () => {
    const { api, added } = fakeDock([
      ["matrix", "matrix"],
      ["details", "details"],
    ]);
    workspace.api = api;
    workspace.refresh();
    workspace.openNew("matrix", "matrix");
    expect(added).toEqual([
      { id: "matrix-2", component: "matrix", title: "Matrix 2", position: { referencePanel: "matrix", direction: "right" } },
    ]);
    workspace.refresh();
    expect(workspace.componentOf("matrix-2")).toBe("matrix");
    expect(workspace.extraInstances("matrix")).toEqual(["matrix-2"]);
    workspace.openNew("matrix", "gone");
    expect(added.at(-1)).toMatchObject({ id: "matrix-3", title: "Matrix 3", position: { direction: "right" } });
    // Single-instance panels get no second instance.
    workspace.openNew("details");
    expect(added).toHaveLength(2);
    workspace.refresh();
    workspace.close("matrix-2");
    workspace.refresh();
    expect(workspace.extraInstances("matrix")).toEqual(["matrix-3"]);
    // The first instance keeps the contribution's id and its checkbox.
    workspace.toggle("matrix");
    workspace.refresh();
    expect(workspace.open.has("matrix")).toBe(false);
    expect(workspace.extraInstances("matrix")).toEqual(["matrix-3"]);
    workspace.api = null;
    workspace.refresh();
  });
});
