import { describe, expect, it, vi } from "vitest";

vi.mock("$lib/components/DetailsPanel.svelte", () => ({ default: {} }));
vi.mock("$lib/components/DiffPanel.svelte", () => ({ default: {} }));
vi.mock("$lib/components/HistorySidebar.svelte", () => ({ default: {} }));
vi.mock("$lib/components/MatrixPanel.svelte", () => ({ default: {} }));
vi.mock("../stores/shown", () => ({ shown: { matrix: null as unknown } }));
vi.mock("../stores/git.svelte", () => ({ git: { historyAvailable: false, diffAvailable: false } }));

import { git } from "../stores/git.svelte";
import { shown } from "../stores/shown";
import "./builtin";
import { panels } from "./registry";

const view = shown as unknown as { matrix: unknown };
const state = git as unknown as { historyAvailable: boolean; diffAvailable: boolean };

describe("built-in panels", () => {
  it("gate the history and diff panels on their own capability", () => {
    state.historyAvailable = true;
    state.diffAvailable = false;
    expect(panels.isAvailable("history")).toBe(true);
    expect(panels.isAvailable("diff")).toBe(false);
    state.historyAvailable = false;
    state.diffAvailable = true;
    expect(panels.isAvailable("history")).toBe(false);
    expect(panels.isAvailable("diff")).toBe(true);
  });

  it("show the matrix once loaded and the details always", () => {
    view.matrix = null;
    expect(panels.isAvailable("matrix")).toBe(false);
    expect(panels.isAvailable("details")).toBe(true);
    view.matrix = {};
    expect(panels.isAvailable("matrix")).toBe(true);
  });
});
