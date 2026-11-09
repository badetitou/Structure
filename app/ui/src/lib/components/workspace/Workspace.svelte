<script lang="ts">
  import { panels } from "../../../workspace/registry";
  import { visible } from "../../../workspace/tiling";
  import { workspace } from "../../../workspace/workspace.svelte";
  import TileNode from "./TileNode.svelte";

  // The tiling workspace: the stored split tree minus the panels that are not available.
  const shown = $derived(visible(workspace.layout, panels.isAvailable));
</script>

<div class="flex min-h-0 min-w-0 flex-1">
  {#if shown}
    <TileNode
      node={shown}
      onresize={(s, a, b, f) => workspace.resize(s, a, b, f)}
      oncommit={() => workspace.save()}
    />
  {/if}
</div>
