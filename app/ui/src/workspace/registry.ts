import type { Component } from "svelte";

/**
 * A panel contributed to the workspace. The layout (tiling.ts) only knows panel ids;
 * everything about what a panel is and shows comes from here.
 */
export interface PanelContribution {
  /** Stable id, stored in the persisted layout. */
  id: string;
  title: string;
  /** Rendered with no props: a panel reads what it needs and acts through `getViewContext()` (context.ts), never the stores. */
  component: Component;
  /** While this returns false, a group holding only unavailable panels is hidden (takes no room); read reactively. */
  available?: () => boolean;
  /**
   * Several instances can be open at once: the first has the contribution's id, the others
   * `<id>-N` (see `nextInstanceId`); the Views menu offers "New <title> view". Single-instance
   * panels (the default) have one panel whose id is the contribution's.
   */
  multi?: boolean;
  /** Each instance owns display options, kept in its dockview params (`ViewContext.forPanel`). */
  display?: boolean;
}

/** Panel registry: contributions by id, in registration order. */
export class PanelRegistry {
  private byId = new Map<string, PanelContribution>();

  /**
   * @param replace a second registration of an id replaces the first instead of throwing
   *   (dev hot reload re-runs the registering module).
   */
  constructor(private readonly replace = false) {}

  /** Adds a contribution; throws when its id is already taken (two plugins clashing). */
  register(c: PanelContribution): void {
    if (this.byId.has(c.id) && !this.replace) throw new Error(`panel "${c.id}" is already registered`);
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
export const panels = new PanelRegistry(import.meta.env.DEV);
