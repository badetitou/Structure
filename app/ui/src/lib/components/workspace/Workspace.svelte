<script lang="ts">
  import { onMount } from "svelte";
  import { createDockview } from "dockview";
  import "dockview/dist/styles/dockview.css";
  import { defaultLayout } from "../../../workspace/builtin";
  import { SvelteRenderer, loadDock, restore, saveDock, structureTheme, syncVisibility } from "../../../workspace/dock";
  import { panels } from "../../../workspace/registry";
  import { workspace } from "../../../workspace/workspace.svelte";
  import MissingPanel from "./MissingPanel.svelte";

  // The dockview workspace: tabs, drag-and-drop docking and resizable splits for
  // the panels in the registry. The layout is restored and saved in localStorage.
  let host: HTMLDivElement;

  onMount(() => {
    const api = createDockview(host, {
      theme: structureTheme,
      createComponent: ({ name }) => new SvelteRenderer(panels.get(name)?.component ?? MissingPanel),
    });
    restore(api, loadDock(), panels.has, defaultLayout);
    workspace.api = api;
    workspace.refresh();

    let timer: ReturnType<typeof setTimeout> | undefined;
    const subs = [
      api.onDidLayoutChange(() => {
        syncVisibility(api, panels.isAvailable);
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
    if (api) syncVisibility(api, panels.isAvailable);
  });
</script>

<div bind:this={host} class="min-h-0 min-w-0 flex-1"></div>
