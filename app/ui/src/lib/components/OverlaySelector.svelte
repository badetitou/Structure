<script lang="ts">
  import XIcon from "@lucide/svelte/icons/x";
  import { isMacPlatform, isToggleClick, selectorVisible } from "../../overlays";
  import { getViewContext } from "../../workspace/context";

  // Chips of the loaded highlight overlays, in a thin bar above the matrix, shown only when
  // there is a choice (two or more, or a kept one left unselected by "Clear impact"). A click
  // shows that overlay alone, cmd-click (macOS) / ctrl-click adds or removes it from the ones
  // drawn together; the x removes it. Locked while an impact runs.
  const { overlays, actions } = getViewContext();
  const list = $derived(overlays.list);
  const mac = typeof navigator !== "undefined" && isMacPlatform(navigator.platform || navigator.userAgent);
  const hint = `click: show only this one · ${mac ? "⌘" : "Ctrl"}-click: add or remove it`;
</script>

{#if selectorVisible(list, overlays.selectedIds)}
  <div
    class="flex min-h-8 flex-none flex-wrap items-center gap-1 border-b bg-card px-2 py-1 whitespace-nowrap"
    role="group"
    aria-label="Highlight overlays"
  >
    <span class="mr-1 flex-none text-xs text-muted-foreground" title={hint}>Overlays</span>
    {#each list as o (o.id)}
      {@const on = overlays.selectedIds.includes(o.id)}
      <span
        class="inline-flex h-6 flex-none items-center rounded-full border text-xs {on
          ? 'border-ring bg-accent text-accent-foreground'
          : 'text-muted-foreground'}"
      >
        <button
          type="button"
          class="flex h-full items-center gap-1.5 rounded-l-full pr-1 pl-2 enabled:hover:text-foreground disabled:cursor-wait"
          aria-pressed={on}
          disabled={overlays.busy}
          title={`${o.pinned ? `${o.label} (kept)` : o.label}${o.stale ? " (rules changed: computed again when shown)" : ""}\n${hint}`}
          onclick={(ev) => actions.selectOverlay(o.id, isToggleClick(ev, mac))}
        >
          <span class="inline-block size-2 flex-none rounded-full" style:background-color="var({o.colour})"></span>
          <code class="max-w-40 truncate">{o.label}</code>
        </button>
        <button
          type="button"
          class="flex h-full items-center rounded-r-full pr-1.5 pl-0.5 enabled:hover:text-foreground disabled:cursor-wait"
          aria-label="Remove overlay {o.label}"
          disabled={overlays.busy}
          onclick={() => actions.removeOverlay(o.id)}><XIcon class="size-3" /></button
        >
      </span>
    {/each}
  </div>
{/if}
