import type { DockviewApi } from "dockview";
import { defaultLayout } from "./builtin";
import { nextInstanceId } from "./paneldisplay";
import { panels } from "./registry";

/** The mounted dockview workspace, for the Views menu (open / close / reset / new view). */
class WorkspaceState {
  api = $state.raw<DockviewApi | null>(null);
  /** Open panels: panel id -> contribution id, kept in step with dockview's add / remove events. */
  open = $state.raw<ReadonlyMap<string, string>>(new Map());

  /** Called on every panel add / remove / layout load. */
  refresh() {
    this.open = new Map(this.api?.panels.map((p) => [p.id, p.api.component] as const) ?? []);
  }

  /** The contribution panel `id` is an instance of (undefined when not open). */
  componentOf = (id: string): string | undefined => this.open.get(id) ?? this.api?.getPanel(id)?.api.component;

  /** Open instances of contribution `contribution` beyond the first (its own id), in open order. */
  extraInstances(contribution: string): string[] {
    return [...this.open].flatMap(([id, c]) => (c === contribution && id !== contribution ? [id] : []));
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

  /**
   * Opens one more instance of multi-instance contribution `contribution` (a new id, `<id>-N`),
   * beside the focused instance when one is open. It starts from the global display defaults.
   */
  openNew(contribution: string, beside: string | null = null) {
    const api = this.api;
    const c = panels.get(contribution);
    if (!api || !c?.multi) return;
    const { id, n } = nextInstanceId(contribution, this.open);
    const ref = beside && api.getPanel(beside) ? beside : null;
    api.addPanel({
      id,
      component: contribution,
      title: `${c.title} ${n}`,
      position: ref ? { referencePanel: ref, direction: "right" } : { direction: "right" },
    });
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
