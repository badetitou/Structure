<script lang="ts">
  import * as api from "./api";
  import { loadRecent, removeRecent, saveRecent } from "./recent";
  import { Button } from "$lib/components/ui/button";
  import * as Card from "$lib/components/ui/card";
  import { Separator } from "$lib/components/ui/separator";

  let {
    path = $bindable(""),
    error = "",
    onAnalyze,
    onPick,
  }: { path?: string; error?: string; onAnalyze: (path: string) => void; onPick?: () => void } = $props();

  let recents = $state(loadRecent());

  async function pick() {
    const p = await api.pickFolder();
    if (p) {
      path = p;
      onPick?.();
    }
  }
  function forget(p: string) {
    saveRecent(removeRecent(loadRecent(), p));
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
      <Card.Description>Supported: Java and TypeScript projects.</Card.Description>
    </Card.Header>
    <Card.Content class="flex flex-col gap-3">
      <Button variant="outline" onclick={pick}>Open project…</Button>
      <div class="truncate text-sm text-muted-foreground">{path || "No project selected"}</div>
      <Button disabled={!path} onclick={() => onAnalyze(path)}>Analyze</Button>
      {#if error}
        <p class="text-sm text-destructive" role="alert">{error}</p>
      {/if}
    </Card.Content>
  </Card.Root>
  {#if recents.length}
    <Separator />
    <h2 class="text-sm font-medium">Recent projects</h2>
    <div class="flex flex-col gap-2">
      {#each recents as r (r.path)}
        <div class="flex gap-2">
          <Button
            variant="outline"
            class="h-auto min-w-0 flex-1 justify-start gap-3 py-2 text-left"
            title={r.path}
            onclick={() => onAnalyze(r.path)}
          >
            <strong>{r.name}</strong>
            <span class="min-w-0 flex-1 truncate font-normal text-muted-foreground">{r.path}</span>
            <span class="font-normal text-muted-foreground">{new Date(r.lastOpened).toLocaleString()}</span>
          </Button>
          <Button
            variant="ghost"
            size="icon"
            title="Remove from recent projects"
            aria-label="Remove from recent projects"
            onclick={() => forget(r.path)}>×</Button
          >
        </div>
      {/each}
    </div>
  {/if}
</div>
