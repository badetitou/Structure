<script lang="ts">
  import { Button } from "$lib/components/ui/button";
  import { ScrollArea } from "$lib/components/ui/scroll-area";
  import { layerLabel } from "../../aligned";
  import { remoteLinks } from "../../graph";
  import { hasMore } from "../../cellsymbols";
  import { initialCaps, moreLabel, showMore } from "../../symbolcap";
  import { symbolName } from "../../symbolname";
  import { cellOverlayRows, overlayViews } from "../../multioverlay";
  import { cellCountsLabel } from "../../statusbar";
  import { getViewContext } from "../../workspace/context";

  const { lens, overlays: impact, selection, actions } = getViewContext();
  const options = lens.display;

  const impacted = $derived(
    impact.current ? [...impact.current.impacted_symbols].sort((a, b) => a.distance - b.distance) : [],
  );
  let caps = $state.raw(initialCaps());
  $effect(() => {
    void impact.current;
    caps = initialCaps();
  });
  // Links between view cells (collapsed domains merged), only while the toolbar toggle is on and
  // the selection is a cell of the active panel's lens (the view's cells).
  const ownCell = $derived(!!selection.selected && !!selection.view && selection.lens === lens.id);
  const http = $derived(
    options.showHttp && ownCell && selection.selected && selection.view ? remoteLinks(selection.view.httpEdges, [selection.selected.id]) : null,
  );
  const rpc = $derived(
    options.showRpc && ownCell && selection.selected && selection.view ? remoteLinks(selection.view.rpcEdges, [selection.selected.id]) : null,
  );
  const viewName = (id: string) => {
    const c = selection.view?.cells.find((x) => x.id === id);
    return c ? `${layerLabel(lens.matrix?.layers, c.layer)} / ${c.label}` : id;
  };
  // Several overlays drawn together: how each one hits the selected cell.
  const overlayRows = $derived.by(() => {
    // The view (and the overlays' cells) are the active panel's lens: only its own cells match.
    if (!impact.multi || !selection.selected || !selection.view || selection.lens !== lens.id) return null;
    const views = overlayViews(impact.selectedOverlays, selection.view.cellMap);
    return cellOverlayRows(impact.selectedOverlays, views.hits.get(selection.selected.id));
  });
  const list = $derived(selection.selected && selection.symbols?.cell === selection.selected.id ? selection.symbols : null);
</script>

{#snippet items(rows: { name: string; sub: string }[], total: number, more?: () => void)}
  {#if total === 0}
    <div class="text-muted-foreground">none</div>
  {:else}
    <ul class="m-0 list-disc pl-4">
      {#each rows as r}
        <li>{r.name}<small class="block break-all text-muted-foreground">{r.sub}</small></li>
      {/each}
    </ul>
    {@const label = more ? moreLabel(rows.length, total) : null}
    {#if more && label}
      <Button variant="outline" size="sm" class="mt-1" onclick={more}>{label}</Button>
    {/if}
  {/if}
{/snippet}

{#snippet heading(text: string)}
  <h3 class="mt-3 mb-1 text-sm font-semibold">{text}</h3>
{/snippet}

{#snippet dot(colour: string)}
  <span class="mr-1.5 inline-block size-2 flex-none rounded-full" style:background-color="var({colour})"></span>
{/snippet}

<ScrollArea class="h-full">
  <div class="px-3.5 py-2.5">
    {#if impact.multi && !selection.selected}
      {@render heading(`Overlays (${impact.selectedOverlays.length})`)}
      <ul class="m-0 list-none pl-0">
        {#each impact.selectedOverlays as o (o.id)}
          <li class="mb-1">
            <span class="flex items-center">{@render dot(o.colour)}<code class="truncate">{o.label}</code></span>
            <small class="block text-muted-foreground"
              >{cellCountsLabel(o.impact)} · {o.impact.changed_symbols.length} changed symbols · {o.impact.impacted_symbols
                .length} impacted symbols</small
            >
          </li>
        {/each}
      </ul>
    {/if}
    {#if impact.current}
      {@render heading(`Changed symbols (${impact.current.changed_symbols.length})`)}
      {@render items(
        impact.current.changed_symbols.slice(0, caps.changed).map((id) => ({ name: symbolName(id), sub: id })),
        impact.current.changed_symbols.length,
        () => (caps = showMore(caps, "changed")),
      )}
      {@render heading(`Impacted symbols (${impact.current.impacted_symbols.length})`)}
      {@render items(
        impacted.slice(0, caps.impacted).map((x) => ({ name: symbolName(x.id), sub: `distance ${x.distance} · ${x.id}` })),
        impacted.length,
        () => (caps = showMore(caps, "impacted")),
      )}
    {/if}
    {#if selection.selected}
      {@const c = selection.selected}
      {@render heading(`${layerLabel(lens.matrix?.layers, c.layer)}/${c.label} — ${c.size} symbols`)}
      {#if overlayRows}
        <ul class="m-0 mb-2 list-none pl-0">
          {#each overlayRows as r (r.id)}
            <li class="flex items-center">
              {@render dot(r.colour)}<code class="truncate">{r.label}</code>
              <span class="ml-auto flex-none pl-2" class:text-muted-foreground={!r.hit}>{r.text}</span>
            </li>
          {/each}
        </ul>
      {/if}
      {#if http && (http.out.length > 0 || http.in.length > 0)}
        {@render heading("HTTP")}
        <ul class="m-0 list-disc pl-4">
          {#each http.out as l (l.other)}
            <li>calls {viewName(l.other)} <small class="text-muted-foreground">({l.count})</small></li>
          {/each}
          {#each http.in as l (l.other)}
            <li>called by {viewName(l.other)} <small class="text-muted-foreground">({l.count})</small></li>
          {/each}
        </ul>
      {/if}
      {#if rpc && (rpc.out.length > 0 || rpc.in.length > 0)}
        {@render heading("RPC")}
        <ul class="m-0 list-disc pl-4">
          {#each rpc.out as l (l.other)}
            <li>calls {viewName(l.other)} <small class="text-muted-foreground">({l.count})</small></li>
          {/each}
          {#each rpc.in as l (l.other)}
            <li>called by {viewName(l.other)} <small class="text-muted-foreground">({l.count})</small></li>
          {/each}
        </ul>
      {/if}
      {#if list && list.items.length > 0}
        {@render items(
          list.items.map((x) => ({ name: x.name, sub: x.file })),
          list.items.length,
        )}
      {/if}
      {#if list?.error}
        <div class="text-muted-foreground">Cannot list symbols: {list.error}</div>
      {:else if !list || list.loading}
        <div class="text-muted-foreground">Loading...</div>
      {:else if hasMore(list)}
        <Button variant="outline" size="sm" onclick={() => void actions.loadMoreSymbols()}>
          Show more ({list.items.length} / {list.total})
        </Button>
      {/if}
    {:else if !impact.current && !impact.multi}
      <div class="text-muted-foreground">Click a cell to list its symbols.</div>
    {/if}
  </div>
</ScrollArea>
