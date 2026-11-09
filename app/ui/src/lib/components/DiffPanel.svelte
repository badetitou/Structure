<script lang="ts">
  import ChevronIcon from "@lucide/svelte/icons/chevron-right";
  import LoaderIcon from "@lucide/svelte/icons/loader-circle";
  import type { DiffLine } from "../../api";
  import { fileLabel, rangeLabel, startsOpen, statusLetter, totals } from "../../diffview";
  import { app } from "../../state.svelte";

  const d = $derived(app.diff);
  const sum = $derived(d?.data ? totals(d.data) : null);

  const rowClass: Record<DiffLine["kind"], string> = {
    add: "bg-diff-add-bg",
    del: "bg-diff-del-bg",
    context: "",
  };
  const marker: Record<DiffLine["kind"], string> = { add: "+", del: "-", context: " " };
</script>

<div class="flex h-full min-h-0 flex-col text-xs">
  {#if !d}
    <div class="px-3.5 py-2.5 text-muted-foreground">Select a commit in the history to see its diff.</div>
  {:else}
    <div class="flex flex-none items-center gap-2 border-b px-3 py-1.5">
      <code class="truncate">{rangeLabel(d.base, d.head)}</code>
      {#if sum}
        <span class="ml-auto flex-none text-muted-foreground">
          {sum.files} file{sum.files === 1 ? "" : "s"}
          <span class="text-diff-add-fg">+{sum.additions}</span>
          <span class="text-diff-del-fg">−{sum.deletions}</span>
        </span>
      {:else if d.loading}
        <LoaderIcon class="ml-auto size-3.5 flex-none animate-spin" />
      {/if}
    </div>
    <div class="min-h-0 flex-1 overflow-auto">
      {#if d.error}
        <div class="px-3.5 py-2.5 text-destructive">Cannot load the diff: {d.error}</div>
      {:else if d.data}
        {#if d.data.truncated}
          <div class="border-b bg-[var(--warn-bg)] px-3 py-1 text-[var(--warn-fg)]">Diff too large: some files or lines are not shown.</div>
        {/if}
        {#if d.data.files.length === 0}
          <div class="px-3.5 py-2.5 text-muted-foreground">No changes under this project.</div>
        {/if}
        {#each d.data.files as f (f.path + "\0" + (f.old_path ?? ""))}
          <details class="group border-b" open={startsOpen(f)}>
            <summary
              class="sticky top-0 z-10 flex cursor-pointer list-none items-center gap-1.5 bg-muted px-2 py-1 select-none [&::-webkit-details-marker]:hidden"
              title={fileLabel(f)}
            >
              <ChevronIcon class="size-3.5 flex-none transition-transform group-open:rotate-90" />
              <code class="w-3 flex-none text-muted-foreground">{statusLetter(f.status)}</code>
              <span class="min-w-0 flex-1 truncate [direction:rtl] text-left"><bdi>{fileLabel(f)}</bdi></span>
              {#if f.binary}
                <span class="flex-none text-muted-foreground">binary</span>
              {:else}
                <span class="flex-none text-diff-add-fg">+{f.additions}</span>
                <span class="flex-none text-diff-del-fg">−{f.deletions}</span>
              {/if}
            </summary>
            {#if f.binary}
              <div class="px-3 py-1.5 text-muted-foreground">Binary file not shown.</div>
            {:else if f.hunks.length === 0}
              <div class="px-3 py-1.5 text-muted-foreground">No textual change (empty file, mode or rename only).</div>
            {:else}
              <table class="w-full border-collapse font-mono leading-5">
                <tbody>
                  {#each f.hunks as h}
                    <tr class="bg-accent/50 text-muted-foreground">
                      <td colspan="3" class="truncate px-2">{h.header}</td>
                    </tr>
                    {#each h.lines as l}
                      <tr class={rowClass[l.kind]}>
                        <td class="w-px px-1.5 text-right whitespace-nowrap text-muted-foreground select-none">{l.old ?? ""}</td>
                        <td class="w-px px-1.5 text-right whitespace-nowrap text-muted-foreground select-none">{l.new ?? ""}</td>
                        <td class="pr-2 whitespace-pre"
                          ><span
                            class="inline-block w-3 select-none {l.kind === 'add'
                              ? 'text-diff-add-fg'
                              : l.kind === 'del'
                                ? 'text-diff-del-fg'
                                : ''}">{marker[l.kind]}</span
                          >{l.text}</td
                        >
                      </tr>
                    {/each}
                  {/each}
                </tbody>
              </table>
              {#if f.truncated}
                <div class="px-3 py-1 text-muted-foreground">… more lines not shown ({f.additions} added, {f.deletions} removed in total).</div>
              {/if}
            {/if}
          </details>
        {/each}
      {/if}
    </div>
  {/if}
</div>
