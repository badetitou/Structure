import "./builtin";
import { panels } from "./registry";
import { loadLayout, resizePair, saveLayout } from "./tiling";
import type { Node } from "./tiling";

/** The workspace layout, persisted across reloads. */
class WorkspaceState {
  layout = $state.raw<Node>(loadLayout(panels.has));

  /** See `resizePair`; not persisted until `save`. */
  resize(splitId: string, a: string, b: string, fraction: number) {
    this.layout = resizePair(this.layout, splitId, a, b, fraction);
  }

  save() {
    saveLayout(this.layout);
  }
}

export const workspace = new WorkspaceState();
