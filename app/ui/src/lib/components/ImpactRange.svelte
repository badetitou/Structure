<script lang="ts">
  import { Popover } from "bits-ui";
  import GitCompare from "@lucide/svelte/icons/git-compare";
  import { Button, buttonVariants } from "$lib/components/ui/button";
  import { Input } from "$lib/components/ui/input";
  import { app } from "../../state.svelte";

  // Typed impact range (base..head) in a popover, out of the way of the dock.
  let open = $state(false);
  let base = $state("");
  let head = $state("");

  function run(e: SubmitEvent) {
    e.preventDefault();
    app.runTypedImpact(base.trim() || null, head.trim() || null);
    open = false;
  }
</script>

<Popover.Root bind:open>
  <Popover.Trigger class={buttonVariants({ variant: "outline", size: "sm" })} aria-label="Impact of a range">
    <GitCompare />Range
  </Popover.Trigger>
  <Popover.Portal>
    <Popover.Content
      align="start"
      sideOffset={4}
      class="z-[1000] w-72 rounded-md border bg-popover p-3 text-popover-foreground shadow-md"
    >
      <form class="flex flex-col gap-2" onsubmit={run}>
        <Input class="h-7" placeholder="base (empty = HEAD)" aria-label="Base ref" bind:value={base} />
        <Input class="h-7" placeholder="head (empty = working tree)" aria-label="Head ref" bind:value={head} />
        <Button type="submit" size="sm" disabled={app.impactBusy}>Show impact</Button>
      </form>
    </Popover.Content>
  </Popover.Portal>
</Popover.Root>
