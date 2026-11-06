<script lang="ts">
  import type { Band, Column } from "../../../domaintree";
  import { DENSE_STEP, PITCH } from "../../../layout";
  import type { Layout } from "../../../layout";
  import { fitText } from "../../../matrixgeom";

  // Group bands (click collapses the group) and column / layer labels (click on
  // a collapsed group expands it). Keyed by column / band, so a collapse or
  // expand only creates or removes the headers that actually appear or go away.
  let {
    lay,
    cols,
    bands,
    bandH,
    top,
    onexpand,
    oncollapse,
  }: {
    lay: Layout;
    cols: Column[];
    bands: Band[];
    bandH: number;
    /** Height of the band rows: the labels sit below them. */
    top: number;
    onexpand: (key: string) => void;
    oncollapse: (key: string) => void;
  } = $props();

  const pitch = $derived(lay.detailed ? PITCH : DENSE_STEP);
  const dense = $derived(!lay.detailed);
</script>

{#each bands as b (b.key)}
  {@const l0 = lay.domainLabels[b.start]}
  {#if l0}
    {@const x = l0.x - pitch / 2}
    {@const w = b.span * pitch}
    <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
    <g class="band" onclick={() => oncollapse(b.key)}>
      <rect x={x + 1} y={b.depth * bandH + 1} width={w - 2} height={bandH - 2} rx="3" />
      <text x={x + w / 2} y={b.depth * bandH + bandH / 2 + 4} text-anchor="middle" font-weight="bold" class:dense>{fitText(`▾ ${b.label}`, w - 6, lay.detailed ? 8 : 6)}</text>
      <title>Collapse {b.label}</title>
    </g>
  {/if}
{/each}

<g transform="translate(0 {top})">
{#each cols as col, i (col.key)}
  {@const l = lay.domainLabels[i]}
  {#if l}
    <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
    <text
      x={l.x}
      y={l.y}
      font-weight="bold"
      class:dense
      class:group={col.group}
      transform={lay.rotateDomains ? `rotate(-90 ${l.x} ${l.y})` : undefined}
      dominant-baseline={lay.rotateDomains ? "middle" : undefined}
      text-anchor={lay.rotateDomains ? undefined : "middle"}
      onclick={col.group ? () => onexpand(col.key) : undefined}>{l.text}</text
    >
  {/if}
{/each}

{#each lay.layerLabels as l (l.text)}
  <text x={l.x} y={l.y} font-weight="bold" class:dense>{l.text}</text>
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
