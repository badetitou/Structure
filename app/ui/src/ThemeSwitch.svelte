<script lang="ts">
  import { userPrefersMode, setMode } from "mode-watcher";
  import * as ToggleGroup from "$lib/components/ui/toggle-group";
  import MonitorIcon from "@lucide/svelte/icons/monitor";
  import SunIcon from "@lucide/svelte/icons/sun";
  import MoonIcon from "@lucide/svelte/icons/moon";

  type Mode = typeof userPrefersMode.current;

  // mode-watcher persists the choice (localStorage) and sets the `dark` class without a flash.
  const value = $derived(userPrefersMode.current);
</script>

<ToggleGroup.Root
  type="single"
  variant="outline"
  size="sm"
  aria-label="Theme"
  {value}
  onValueChange={(v) => v && setMode(v as Mode)}
>
  <ToggleGroup.Item value="system" aria-label="Follow system theme"><MonitorIcon /></ToggleGroup.Item>
  <ToggleGroup.Item value="light" aria-label="Light theme"><SunIcon /></ToggleGroup.Item>
  <ToggleGroup.Item value="dark" aria-label="Dark theme"><MoonIcon /></ToggleGroup.Item>
</ToggleGroup.Root>
