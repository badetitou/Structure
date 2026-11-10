<script lang="ts">
  import type { Band, Column } from "../../../domaintree";
  import type { Layout } from "../../../layout";
  import { fitText } from "../../../matrixgeom";

  // Group bands (click collapses the group) and column / layer labels (click on
  // a collapsed group expands it). Keyed by column / band, so a collapse or
  // expand only creates or removes the headers that actually appear or go away.
  let {
    lay,
    heads,
    bands,
    bandH,
    top,
    onexpand,
    oncollapse,
  }: {
    lay: Layout;
    /** One header per domain slot: the columns it holds (several in the aligned view). */
    heads: { key: string; members: Column[] }[];
    bands: Band[];
    bandH: number;
    /** Depth of the band rows: the labels sit below them (right of them when transposed). */
    top: number;
    onexpand: (key: string) => void;
    oncollapse: (key: string) => void;
  } = $props();

  const dense = $derived(!lay.detailed && !lay.flame);
  // Glyph width used to cut labels to their band / column.
  const charW = $derived(lay.detailed || lay.flame ? 7 : 6);
</script>

{#each bands as b (b.key)}
  {@const c0 = lay.columns[b.start]}
  {@const cn = lay.columns[b.start + b.span - 1]}
  {#if c0 && cn}
    {@const u = c0.x}
    {@const len = cn.x + cn.w - c0.x}
    {@const text = fitText(`▾ ${b.label}`, len - 6, lay.detailed ? 8 : charW)}
    <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
    <g class="band" onclick={() => oncollapse(b.key)}>
      {#if lay.transposed}
        {@const cx = b.depth * bandH + bandH / 2}
        {@const cy = u + len / 2}
        <rect x={b.depth * bandH + 1} y={u + 1} width={bandH - 2} height={len - 2} rx="3" />
        <text x={cx} y={cy} transform="rotate(-90 {cx} {cy})" text-anchor="middle" dominant-baseline="middle" font-weight="bold" class:dense>{text}</text>
      {:else}
        <rect x={u + 1} y={b.depth * bandH + 1} width={len - 2} height={bandH - 2} rx="3" />
        <text x={u + len / 2} y={b.depth * bandH + bandH / 2 + 4} text-anchor="middle" font-weight="bold" class:dense>{text}</text>
      {/if}
      <title>Collapse {b.label}</title>
    </g>
  {/if}
{/each}

<g transform={lay.transposed ? `translate(${top} 0)` : `translate(0 ${top})`}>
{#each heads as head, i (head.key)}
  {@const l = lay.domainLabels[i]}
  {#if l}
    <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
    <text
      x={l.x}
      y={l.y}
      font-weight="bold"
      class:dense
      transform={lay.rotateDomains ? `rotate(-${lay.angle} ${l.x} ${l.y})` : undefined}
      dominant-baseline={lay.rotateDomains ? "middle" : undefined}
      text-anchor={lay.rotateDomains || lay.transposed ? undefined : "middle"}
      >{#if lay.flame && !lay.transposed}{@const only = head.members.length === 1 ? head.members[0] : undefined}<tspan class:group={only?.group} onclick={only?.group ? () => onexpand(only.key) : undefined}>{fitText(l.text, (lay.columns[i]?.w ?? 0) - 2, charW)}</tspan>{:else}{#each head.members as col, j (col.key)}{#if j > 0}{" ⇄ "}{/if}<tspan class:group={col.group} onclick={col.group ? () => onexpand(col.key) : undefined}>{col.group ? "▸ " : ""}{col.label}</tspan>{/each}{/if}<title>{l.text}</title></text
    >
  {/if}
{/each}

{#each lay.layerLabels as l, i (i)}
  <text
    x={l.x}
    y={l.y}
    font-weight="bold"
    class:dense
    transform={lay.rotateLayers ? `rotate(-${lay.angle} ${l.x} ${l.y})` : undefined}
    dominant-baseline={lay.rotateLayers ? "middle" : undefined}
    text-anchor={lay.transposed && !lay.rotateLayers ? "middle" : undefined}
    >{lay.flame && lay.transposed ? fitText(l.text, (lay.rows[i]?.w ?? 0) - 4, charW) : l.text}<title>{l.text}</title></text
  >
{/each}
</g>

<style>
  text {
    fill: var(--foreground);
    font-size: 11px;
  }
  .dense {
    font-size: 10px;
  }
  .group {
    cursor: pointer;
  }
  .band {
    cursor: pointer;
  }
  .band rect {
    fill: var(--band-fill);
    stroke: var(--band-stroke);
  }
  .band text {
    pointer-events: none;
  }
</style>
