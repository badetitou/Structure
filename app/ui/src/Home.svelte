<script lang="ts">
  import * as api from "./api";
  import { loadRecent, recentKey, removeRecent, saveRecent } from "./recent";
  import { Button } from "$lib/components/ui/button";
  import * as Card from "$lib/components/ui/card";
  import { Separator } from "$lib/components/ui/separator";

  let {
    paths = $bindable([]),
    error = "",
    onAnalyze,
    onPick,
  }: {
    paths?: string[];
    error?: string;
    onAnalyze: (paths: string[]) => void;
    onPick?: () => void;
  } = $props();

  let recents = $state(loadRecent());
  // Hosts without a folder picker (none shows Home today) only offer the recent projects.
  const canPick = api.capabilities().pickFolder;

  async function add() {
    const picked = await api.pickFolder();
    // Strip trailing separators so "/a/b/" and "/a/b" count as one folder.
    const p = picked && (picked.replace(/[\\/]+$/, "") || picked);
    if (p) {
      if (!paths.includes(p)) paths = [...paths, p];
      onPick?.();
    }
  }
  function drop(p: string) {
    paths = paths.filter((x) => x !== p);
  }
  function forget(ps: string[]) {
    saveRecent(removeRecent(loadRecent(), ps));
    recents = loadRecent();
  }
</script>

<div class="mx-auto flex max-w-xl flex-col gap-4 px-4 py-16">
  <div class="flex items-center gap-4">
    <img src="/logo.svg" alt="" class="size-16" />
    <h1 class="text-3xl font-semibold">Structure</h1>
  </div>
  <Card.Root>
    <Card.Header>
      <Card.Title>Analyze a project</Card.Title>
      <Card.Description>
        Supported: Java and TypeScript projects. Add several folders (e.g. front and back) to analyze them together.
      </Card.Description>
    </Card.Header>
    <Card.Content class="flex flex-col gap-3">
      {#if canPick}
        <Button variant="outline" onclick={add}>{paths.length ? "Add another folder…" : "Open project…"}</Button>
      {/if}
      {#if paths.length}
        <ul class="flex flex-col gap-1">
          {#each paths as p (p)}
            <li class="flex items-center gap-2 text-sm text-muted-foreground">
              <span class="min-w-0 flex-1 truncate" title={p}>{p}</span>
              <Button
                variant="ghost"
                size="icon"
                title="Remove this folder"
                aria-label="Remove folder {p}"
                onclick={() => drop(p)}>×</Button
              >
            </li>
          {/each}
        </ul>
      {:else}
        <div class="text-sm text-muted-foreground">No project selected</div>
      {/if}
      <Button disabled={paths.length === 0} onclick={() => onAnalyze(paths)}>Analyze</Button>
      {#if error}
        <p class="text-sm text-destructive" role="alert">{error}</p>
      {/if}
    </Card.Content>
  </Card.Root>
  {#if recents.length}
    <Separator />
    <h2 class="text-sm font-medium">Recent projects</h2>
    <div class="flex flex-col gap-2">
      {#each recents as r (recentKey(r.paths))}
        <div class="flex gap-2">
          <Button
            variant="outline"
            class="h-auto min-w-0 flex-1 justify-start gap-3 py-2 text-left"
            title={r.paths.join("\n")}
            onclick={() => onAnalyze(r.paths)}
          >
            <strong>{r.name}</strong>
            <span class="min-w-0 flex-1 truncate font-normal text-muted-foreground">{r.paths.join(", ")}</span>
            <span class="font-normal text-muted-foreground">{new Date(r.lastOpened).toLocaleString()}</span>
          </Button>
          <Button
            variant="ghost"
            size="icon"
            title="Remove from recent projects"
            aria-label="Remove from recent projects"
            onclick={() => forget(r.paths)}>×</Button
          >
        </div>
      {/each}
    </div>
  {/if}
</div>
