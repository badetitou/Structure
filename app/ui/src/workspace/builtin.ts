import type { DockviewApi } from "dockview";
import DetailsPanel from "$lib/components/DetailsPanel.svelte";
import DiffPanel from "$lib/components/DiffPanel.svelte";
import HistorySidebar from "$lib/components/HistorySidebar.svelte";
import MatrixPanel from "$lib/components/MatrixPanel.svelte";
import { git } from "../stores/git.svelte";
import { shown } from "../stores/shown";
import { panels } from "./registry";

// The panels Structure ships with. A new panel is one more `register` call
// (here or in its own module imported once at startup).
panels.register({ id: "matrix", title: "Matrix", component: MatrixPanel, available: () => shown.matrix !== null });
panels.register({ id: "details", title: "Details", component: DetailsPanel });
panels.register({ id: "history", title: "History", component: HistorySidebar, available: () => git.historyAvailable });
panels.register({ id: "diff", title: "Diff", component: DiffPanel, available: () => git.diffAvailable });

/** History | matrix | details, with the diff as a tab beside details. */
export function defaultLayout(api: DockviewApi): void {
  api.addPanel({ id: "matrix", component: "matrix", title: "Matrix" });
  api.addPanel({
    id: "history",
    component: "history",
    title: "History",
    position: { referencePanel: "matrix", direction: "left" },
    initialWidth: 260,
  });
  api.addPanel({
    id: "details",
    component: "details",
    title: "Details",
    position: { referencePanel: "matrix", direction: "right" },
    initialWidth: 340,
  });
  api.addPanel({
    id: "diff",
    component: "diff",
    title: "Diff",
    position: { referencePanel: "details", direction: "within" },
    inactive: true,
  });
  // `initialWidth` only applies when the group is created with room to spare; set it explicitly.
  api.getPanel("history")?.group.api.setSize({ width: 260 });
  api.getPanel("details")?.group.api.setSize({ width: 340 });
  api.getPanel("matrix")?.api.setActive();
}
