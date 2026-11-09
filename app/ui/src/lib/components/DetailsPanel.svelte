<script lang="ts">
  import { Button } from "$lib/components/ui/button";
  import { ScrollArea } from "$lib/components/ui/scroll-area";
  import { httpLinks } from "../../graph";
  import { hasMore } from "../../cellsymbols";
  import { initialCaps, moreLabel, showMore } from "../../symbolcap";
  import { symbolName } from "../../symbolname";
  import { app } from "../../state.svelte";

  const impacted = $derived(
    app.impact ? [...app.impact.impacted_symbols].sort((a, b) => a.distance - b.distance) : [],
  );
  let shown = $state.raw(initialCaps());
  $effect(() => {
    void app.impact;
    shown = initialCaps();
  });
  const http = $derived(app.selected && app.matrix ? httpLinks(app.matrix.httpEdges, app.selected.members) : null);
  const list = $derived(app.selected && app.symbols?.cell === app.selected.id ? app.symbols : null);
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

<ScrollArea class="h-full">
  <div class="px-3.5 py-2.5">
    {#if app.impact}
      {@render heading(`Changed symbols (${app.impact.changed_symbols.length})`)}
      {@render items(
        app.impact.changed_symbols.slice(0, shown.changed).map((id) => ({ name: symbolName(id), sub: id })),
        app.impact.changed_symbols.length,
        () => (shown = showMore(shown, "changed")),
      )}
      {@render heading(`Impacted symbols (${app.impact.impacted_symbols.length})`)}
      {@render items(
        impacted.slice(0, shown.impacted).map((x) => ({ name: symbolName(x.id), sub: `distance ${x.distance} · ${x.id}` })),
        impacted.length,
        () => (shown = showMore(shown, "impacted")),
      )}
    {/if}
    {#if app.selected}
      {@const c = app.selected}
      {@render heading(`${c.layer}/${c.label} — ${c.size} symbols`)}
      {#if http && (http.out.length > 0 || http.in.length > 0)}
        {@render heading("HTTP")}
        <ul class="m-0 list-disc pl-4">
          {#each http.out as l (l.other)}
            <li>calls {l.other} <small class="text-muted-foreground">({l.count})</small></li>
          {/each}
          {#each http.in as l (l.other)}
            <li>called by {l.other} <small class="text-muted-foreground">({l.count})</small></li>
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
        <Button variant="outline" size="sm" onclick={() => void app.loadSymbols(true)}>
          Show more ({list.items.length} / {list.total})
        </Button>
      {/if}
    {:else if !app.impact}
      <div class="text-muted-foreground">Click a cell to list its symbols.</div>
    {/if}
  </div>
</ScrollArea>
