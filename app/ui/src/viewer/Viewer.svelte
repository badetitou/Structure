<script lang="ts">
  import { ModeWatcher } from "mode-watcher";
  import AnalysisScreen from "$lib/components/AnalysisScreen.svelte";

  // The analysis screen alone: no Home, no way back; the impact is shown on load.
  let { onError }: { onError: (error?: string) => void } = $props();
</script>

<ModeWatcher />
<AnalysisScreen paths={["viewer"]} embedded onExit={onError} />
