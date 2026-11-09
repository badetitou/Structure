<script lang="ts">
  import { panels } from "../../../workspace/registry";
  import type { Leaf } from "../../../workspace/tiling";

  // One tile: a title bar with its panels, then the active panel.
  let { leaf }: { leaf: Leaf } = $props();

  const active = $derived(panels.get(leaf.active));
</script>

<section class="flex min-h-0 min-w-0 flex-1 flex-col overflow-hidden bg-card text-[13px]" data-tile={leaf.id}>
  <header class="flex h-7 flex-none items-center gap-1 border-b px-2 text-xs select-none">
    {#each leaf.panels as id (id)}
      <span
        class="truncate px-1.5 py-0.5 {id === leaf.active ? 'font-semibold text-foreground' : 'text-muted-foreground'}"
      >
        {panels.get(id)?.title ?? id}
      </span>
    {/each}
  </header>
  <div class="flex min-h-0 flex-1 flex-col">
    {#if active}
      <active.component />
    {/if}
  </div>
</section>
