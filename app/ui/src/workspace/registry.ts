import type { Component } from "svelte";

/**
 * A panel contributed to the workspace. The layout (tiling.ts) only knows panel ids;
 * everything about what a panel is and shows comes from here.
 */
export interface PanelContribution {
  /** Stable id, stored in the persisted layout. */
  id: string;
  title: string;
  /** Rendered with no props: a panel reads what it needs from the app state. */
  component: Component;
  /** Hidden (its tile takes no room) while this returns false; read reactively. */
  available?: () => boolean;
}

/** Panel registry: contributions by id, in registration order. */
export class PanelRegistry {
  private byId = new Map<string, PanelContribution>();

  /** Adds a contribution; throws when its id is already taken (two plugins clashing). */
  register(c: PanelContribution): void {
    if (this.byId.has(c.id)) throw new Error(`panel "${c.id}" is already registered`);
    this.byId.set(c.id, c);
  }

  get(id: string): PanelContribution | undefined {
    return this.byId.get(id);
  }

  has = (id: string): boolean => this.byId.has(id);

  /** Registered and currently available. */
  isAvailable = (id: string): boolean => {
    const c = this.byId.get(id);
    return c !== undefined && (c.available?.() ?? true);
  };

  all(): PanelContribution[] {
    return [...this.byId.values()];
  }
}

/** The app's registry; built-in panels register in builtin.ts. */
export const panels = new PanelRegistry();
