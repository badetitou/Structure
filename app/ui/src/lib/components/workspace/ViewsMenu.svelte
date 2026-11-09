<script lang="ts">
  import { DropdownMenu } from "bits-ui";
  import Check from "@lucide/svelte/icons/check";
  import { buttonVariants } from "$lib/components/ui/button";
  import { panels } from "../../../workspace/registry";
  import { workspace } from "../../../workspace/workspace.svelte";

  // Lists every registered panel: checked = open. Toggling opens / closes it;
  // "Reset layout" restores the default arrangement.
  const item =
    "flex cursor-default items-center gap-2 rounded-sm px-2 py-1 text-sm outline-none select-none data-highlighted:bg-accent data-disabled:opacity-50";
</script>

<DropdownMenu.Root>
  <DropdownMenu.Trigger class={buttonVariants({ variant: "outline", size: "sm" })}>Views</DropdownMenu.Trigger>
  <DropdownMenu.Portal>
    <DropdownMenu.Content
      align="end"
      sideOffset={4}
      class="z-[1000] min-w-44 rounded-md border bg-popover p-1 text-popover-foreground shadow-md"
    >
      {#each panels.all() as c (c.id)}
        <DropdownMenu.CheckboxItem
          class={item}
          checked={workspace.open.has(c.id)}
          disabled={!(c.available?.() ?? true)}
          onCheckedChange={() => workspace.toggle(c.id)}
          closeOnSelect={false}
        >
          {#snippet children({ checked })}
            <span class="flex size-4 items-center justify-center">{#if checked}<Check class="size-3.5" />{/if}</span>
            {c.title}
          {/snippet}
        </DropdownMenu.CheckboxItem>
      {/each}
      <DropdownMenu.Separator class="my-1 h-px bg-border" />
      <DropdownMenu.Item class={item} onSelect={() => workspace.reset()}>
        <span class="size-4"></span>Reset layout
      </DropdownMenu.Item>
    </DropdownMenu.Content>
  </DropdownMenu.Portal>
</DropdownMenu.Root>
