<script lang="ts">
  import LoaderIcon from "@lucide/svelte/icons/loader-circle";
  import { Badge } from "$lib/components/ui/badge";
  import { ScrollArea } from "$lib/components/ui/scroll-area";
  import { rowSvg } from "../../graphsvg";
  import { clickCommit, headIndex, isSelected, relativeDate } from "../../history";
  import { layoutLanes } from "../../lanes";
  import { git } from "../../stores/git.svelte";
  import { impact } from "../../stores/impact.svelte";
  import { pickHistory } from "../../stores/session";

  const hist = $derived(git.history);
  const lanes = $derived(hist ? layoutLanes(hist.commits) : null);
  const headColor = $derived(lanes?.rows[0]?.color);

  /** Mounts a commit-graph row (SVG built by graphsvg.ts) into `node`. */
  const graph = (build: () => SVGElement) => (node: HTMLElement) => {
    node.replaceChildren(build());
  };
  const rowClass =
    "flex h-10 cursor-pointer items-center gap-1.5 overflow-hidden px-2.5 pl-1.5 select-none enabled:hover:bg-muted disabled:cursor-wait";
  // Rows are locked while an impact runs; the selected (pending) rows get a spinner.
  const busy = $derived(impact.busy);
  const dim = $derived(busy ? "opacity-60" : "");
  // Only the range's head is analysed; the rest of the range is just the diff base.
  const pending = $derived(busy ? headIndex(git.historySel) : null);
</script>

{#if hist && lanes}
  <ScrollArea class="h-full text-xs">
    {#if hist.dirty}
      {@const first = lanes.rows[0]}
      <button
        type="button"
        class="{rowClass} w-full text-left {git.historySel?.kind === 'worktree' ? 'bg-accent' : dim}"
        disabled={busy}
        aria-busy={busy && git.historySel?.kind === "worktree"}
        onclick={() => pickHistory({ kind: "worktree" })}
      >
        <span
          class="flex-none"
          {@attach graph(() =>
            rowSvg(first ? { ...first, top: [], through: [], bottom: [] } : null, lanes.width, {
              hollow: true,
              lineBelow: headColor,
            }),
          )}
        ></span>
        <span class="min-w-0 flex-1">
          <span class="block truncate">Working tree</span>
          <span class="mt-0.5 block truncate text-muted-foreground">uncommitted changes</span>
        </span>
        {#if busy && git.historySel?.kind === "worktree"}<LoaderIcon class="size-3.5 flex-none animate-spin" />{/if}
      </button>
    {/if}
    {#each hist.commits as c, i (c.id)}
      <button
        type="button"
        class="{rowClass} w-full text-left {isSelected(git.historySel, i) ? 'bg-accent' : dim}"
        disabled={busy}
        aria-busy={pending === i}
        onclick={(ev) => pickHistory(clickCommit(git.historySel, i, ev.shiftKey))}
      >
        <span
          class="flex-none"
          {@attach graph(() => rowSvg(lanes.rows[i] ?? null, lanes.width, { lineAbove: i === 0 && hist.dirty ? headColor : undefined }))}
        ></span>
        <span class="min-w-0 flex-1">
          <span class="block truncate" title={c.subject}><code>{c.short}</code> {c.subject}</span>
          <span class="mt-0.5 block truncate text-muted-foreground">
            {c.author} · {relativeDate(c.date)}
            {#each c.refs as ref}<Badge variant="secondary" class="ml-1 h-4 rounded-full px-1.5 text-[10px]">{ref}</Badge>{/each}
          </span>
        </span>
        {#if pending === i}<LoaderIcon class="size-3.5 flex-none animate-spin" />{/if}
      </button>
    {/each}
  </ScrollArea>
{/if}
