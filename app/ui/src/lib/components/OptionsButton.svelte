<script lang="ts">
  import SettingsIcon from "@lucide/svelte/icons/settings";
  import XIcon from "@lucide/svelte/icons/x";
  import { Button } from "$lib/components/ui/button";
  import * as ToggleGroup from "$lib/components/ui/toggle-group";
  import ThemeSwitch from "../../ThemeSwitch.svelte";
  import { analysis } from "../../stores/analysis.svelte";
  import { options } from "../../stores/options.svelte";
  import { reloadRules } from "../../stores/session";

  // Gear button that opens the Options page: app preferences, persisted as they change.
  let open = $state(false);

  function onkeydown(e: KeyboardEvent) {
    if (open && e.key === "Escape") open = false;
  }
</script>

<svelte:window {onkeydown} />

<Button variant="ghost" size="icon-sm" aria-label="Options" title="Options" onclick={() => (open = true)}
  ><SettingsIcon /></Button
>

{#if open}
  <div class="fixed inset-0 z-50 flex flex-col bg-background" role="dialog" aria-modal="true" aria-label="Options">
    <div class="flex h-10 flex-none items-center justify-between border-b bg-card px-4">
      <h1 class="m-0 text-sm font-semibold">Options</h1>
      <Button variant="ghost" size="icon-sm" aria-label="Close options" onclick={() => (open = false)}><XIcon /></Button>
    </div>
    <div class="mx-auto flex w-full max-w-xl flex-col gap-6 overflow-y-auto p-6 text-sm">
      <section class="flex flex-col gap-3">
        <h2 class="m-0 text-xs font-semibold tracking-wide text-muted-foreground uppercase">Appearance</h2>
        <div class="flex items-center justify-between gap-4">
          <span>Theme</span>
          <ThemeSwitch />
        </div>
      </section>
      {#if analysis.rulesReloadable}
        <section class="flex flex-col gap-3">
          <h2 class="m-0 text-xs font-semibold tracking-wide text-muted-foreground uppercase">Project</h2>
          <div class="flex items-center justify-between gap-4">
            <span class="flex flex-col">
              <span>Classification rules</span>
              <span class="text-xs text-muted-foreground"
                >Apply <code>.structure/rules.toml</code> again without re-analysing (done automatically when it changes).</span
              >
            </span>
            <Button variant="outline" size="sm" disabled={analysis.rulesBusy} onclick={() => void reloadRules()}
              >{analysis.rulesBusy ? "Reloading…" : "Reload rules"}</Button
            >
          </div>
          {#if analysis.rulesError}<p class="m-0 text-xs text-destructive">{analysis.rulesError}</p>{/if}
        </section>
      {/if}
      <section class="flex flex-col gap-3">
        <h2 class="m-0 text-xs font-semibold tracking-wide text-muted-foreground uppercase">Matrix</h2>
        <div class="flex items-center justify-between gap-4">
          <span class="flex flex-col">
            <span>Top labels</span>
            <span class="text-xs text-muted-foreground">Dense view: column labels upright or at 45°.</span>
          </span>
          <ToggleGroup.Root
            type="single"
            variant="outline"
            size="sm"
            aria-label="Top labels angle"
            value={options.angledLabels ? "45" : "90"}
            onValueChange={(v) => v && options.setAngledLabels(v === "45")}
          >
            <ToggleGroup.Item value="90" class="px-2">Vertical</ToggleGroup.Item>
            <ToggleGroup.Item value="45" class="px-2">45°</ToggleGroup.Item>
          </ToggleGroup.Root>
        </div>
      </section>
    </div>
  </div>
{/if}
