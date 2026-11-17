<script lang="ts">
  import XIcon from "@lucide/svelte/icons/x";
  import { selectorVisible } from "../../overlays";
  import { impact } from "../../stores/impact.svelte";
  import { removeOverlay, selectOverlay } from "../../stores/session";

  // Chips of the loaded highlight overlays, shown only when there is a choice (two or more):
  // a click shows that overlay, the x removes it. Locked while an impact runs.
  const list = $derived(impact.overlays);
</script>

{#if selectorVisible(list)}
  <div class="flex flex-none items-center gap-1" role="group" aria-label="Highlight overlays">
    {#each list as o (o.id)}
      {@const on = o.id === impact.selectedId}
      <span
        class="inline-flex h-6 items-center rounded-full border text-xs {on
          ? 'border-ring bg-accent text-accent-foreground'
          : 'text-muted-foreground'}"
      >
        <button
          type="button"
          class="flex h-full items-center gap-1.5 rounded-l-full pr-1 pl-2 enabled:hover:text-foreground disabled:cursor-wait"
          aria-pressed={on}
          disabled={impact.busy}
          title={o.pinned ? `${o.label} (kept)` : o.label}
          onclick={() => selectOverlay(o.id)}
        >
          <span class="inline-block size-2 flex-none rounded-full" style:background-color="var({o.colour})"></span>
          <code class="max-w-40 truncate">{o.label}</code>
        </button>
        <button
          type="button"
          class="flex h-full items-center rounded-r-full pr-1.5 pl-0.5 enabled:hover:text-foreground disabled:cursor-wait"
          aria-label="Remove overlay {o.label}"
          disabled={impact.busy}
          onclick={() => removeOverlay(o.id)}><XIcon class="size-3" /></button
        >
      </span>
    {/each}
  </div>
{/if}
