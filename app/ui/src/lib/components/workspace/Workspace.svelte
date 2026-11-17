<script lang="ts">
  import { onMount } from "svelte";
  import { createDockview, type DockviewApi } from "dockview";
  import "dockview/dist/styles/dockview.css";
  import { defaultLayout } from "../../../workspace/builtin";
  import { SvelteRenderer, byComponent, dockDnd, loadDock, restore, saveDock, setTabHiddenIn, structureTheme, syncVisibility } from "../../../workspace/dock";
  import { getViewContext, viewContextMap } from "../../../workspace/context";
  import { displayParams } from "../../../workspace/paneldisplay";
  import { panels } from "../../../workspace/registry";
  import { workspace } from "../../../workspace/workspace.svelte";
  import MissingPanel from "./MissingPanel.svelte";

  // The dockview workspace: tabs, drag-and-drop docking and resizable splits for
  // the panels in the registry. The layout is restored and saved in localStorage.
  let host: HTMLDivElement;
  // Panels are mounted as their own Svelte roots: they get the view context through `mount`.
  // A panel owning display options (a matrix panel) gets its own context, its options kept in
  // its dockview params and saved with the layout.
  const ctx = getViewContext();
  const context = viewContextMap(ctx);
  // Availability and "known" are asked of a panel's contribution (instances share one).
  const available = byComponent(workspace.componentOf, panels.isAvailable);

  const sync = (api: DockviewApi) =>
    syncVisibility(api, available, (id, hidden) => setTabHiddenIn(host, id, hidden));

  onMount(() => {
    let timer: ReturnType<typeof setTimeout> | undefined;
    let live = true;
    const scheduleSave = () => {
      clearTimeout(timer);
      timer = setTimeout(() => live && saveDock(dock), 300);
    };
    const dock = createDockview(host, {
      theme: structureTheme,
      ...dockDnd,
      createComponent: ({ id, name }) => {
        const c = panels.get(name);
        if (!c?.display) return new SvelteRenderer(c?.component ?? MissingPanel, context);
        return new SvelteRenderer(c.component, (init) =>
          viewContextMap(
            ctx.forPanel(id, init.params, (d) =>
              // Deferred: the first write can come while dockview is still creating the panel.
              queueMicrotask(() => {
                if (!live) return;
                init.api.updateParameters(displayParams(d));
                scheduleSave();
              }),
            ),
          ),
        );
      },
    });
    restore(dock, loadDock(), byComponent((id) => dock.getPanel(id)?.api.component, panels.has), defaultLayout);
    // Panels a rejected layout created then cleared (before the subscriptions below).
    for (const id of [...ctx.displays.recent]) if (!dock.getPanel(id)) ctx.displays.drop(id);
    workspace.api = dock;
    workspace.refresh();
    if (dock.activePanel) ctx.displays.focus(dock.activePanel.id);

    const subs = [
      dock.onDidLayoutChange(() => {
        sync(dock);
        scheduleSave();
      }),
      dock.onDidActivePanelChange((e) => {
        if (e.panel) ctx.displays.focus(e.panel.id);
      }),
      dock.onDidAddPanel(() => workspace.refresh()),
      dock.onDidRemovePanel((p) => {
        ctx.displays.drop(p.id);
        workspace.refresh();
      }),
      dock.onDidLayoutFromJSON(() => workspace.refresh()),
    ];
    return () => {
      live = false;
      clearTimeout(timer);
      saveDock(dock);
      subs.forEach((s) => s.dispose());
      workspace.api = null;
      workspace.refresh();
      dock.dispose();
      ctx.displays.clear();
    };
  });

  // Re-run when a panel's availability changes (e.g. history arrives after the matrix).
  $effect(() => {
    const api = workspace.api;
    if (api) sync(api);
  });
</script>

<div bind:this={host} class="min-h-0 min-w-0 flex-1"></div>
