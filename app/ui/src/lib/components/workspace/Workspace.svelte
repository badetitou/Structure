<script lang="ts">
  import { onMount } from "svelte";
  import { createDockview, type DockviewApi } from "dockview";
  import "dockview/dist/styles/dockview.css";
  import { defaultLayout } from "../../../workspace/builtin";
  import { SvelteRenderer, dockDnd, loadDock, restore, saveDock, setTabHiddenIn, structureTheme, syncVisibility } from "../../../workspace/dock";
  import { getViewContext, viewContextMap } from "../../../workspace/context";
  import { panels } from "../../../workspace/registry";
  import { workspace } from "../../../workspace/workspace.svelte";
  import MissingPanel from "./MissingPanel.svelte";

  // The dockview workspace: tabs, drag-and-drop docking and resizable splits for
  // the panels in the registry. The layout is restored and saved in localStorage.
  let host: HTMLDivElement;
  // Panels are mounted as their own Svelte roots: they get the view context through `mount`.
  const context = viewContextMap(getViewContext());

  const sync = (api: DockviewApi) =>
    syncVisibility(api, panels.isAvailable, (id, hidden) => setTabHiddenIn(host, id, hidden));

  onMount(() => {
    const api = createDockview(host, {
      theme: structureTheme,
      ...dockDnd,
      createComponent: ({ name }) => new SvelteRenderer(panels.get(name)?.component ?? MissingPanel, context),
    });
    restore(api, loadDock(), panels.has, defaultLayout);
    workspace.api = api;
    workspace.refresh();

    let timer: ReturnType<typeof setTimeout> | undefined;
    const subs = [
      api.onDidLayoutChange(() => {
        sync(api);
        clearTimeout(timer);
        timer = setTimeout(() => saveDock(api), 300);
      }),
      api.onDidAddPanel(() => workspace.refresh()),
      api.onDidRemovePanel(() => workspace.refresh()),
      api.onDidLayoutFromJSON(() => workspace.refresh()),
    ];
    return () => {
      clearTimeout(timer);
      saveDock(api);
      subs.forEach((s) => s.dispose());
      workspace.api = null;
      workspace.refresh();
      api.dispose();
    };
  });

  // Re-run when a panel's availability changes (e.g. history arrives after the matrix).
  $effect(() => {
    const api = workspace.api;
    if (api) sync(api);
  });
</script>

<div bind:this={host} class="min-h-0 min-w-0 flex-1"></div>
