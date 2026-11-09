import type { DockviewApi } from "dockview";
import { defaultLayout } from "./builtin";
import { panels } from "./registry";

/** The mounted dockview workspace, for the Views menu (open / close / reset). */
class WorkspaceState {
  api = $state.raw<DockviewApi | null>(null);
  /** Ids of the open panels, kept in step with dockview's add / remove events. */
  open = $state.raw<ReadonlySet<string>>(new Set());

  /** Called on every panel add / remove / layout load. */
  refresh() {
    this.open = new Set(this.api?.panels.map((p) => p.id) ?? []);
  }

  /** Opens `id` on the right of the workspace, or focuses it when already open. */
  show(id: string) {
    const api = this.api;
    const c = panels.get(id);
    if (!api || !c) return;
    const existing = api.getPanel(id);
    if (existing) existing.api.setActive();
    else api.addPanel({ id, component: id, title: c.title, position: { direction: "right" }, initialWidth: 320 });
  }

  close(id: string) {
    this.api?.getPanel(id)?.api.close();
  }

  toggle(id: string) {
    if (this.open.has(id)) this.close(id);
    else this.show(id);
  }

  reset() {
    const api = this.api;
    if (!api) return;
    api.clear();
    defaultLayout(api);
  }
}

export const workspace = new WorkspaceState();
