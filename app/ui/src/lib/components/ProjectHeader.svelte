<script lang="ts">
  import { Button } from "$lib/components/ui/button";
  import type { Matrix } from "../../api";

  let { matrix, onClose }: { matrix: Matrix; onClose: () => void } = $props();
  const st = $derived(matrix.stats);
</script>

<header class="flex flex-none items-center gap-4 border-b bg-card px-4 py-2.5">
  <h1 class="m-0 text-lg font-semibold">{matrix.project}</h1>
  <span class="text-sm text-muted-foreground"
    >{matrix.language} · {st.files} files · {st.symbols} symbols · {st.edges} edges · {st.elapsed_ms} ms</span
  >
  <Button variant="outline" size="sm" onclick={onClose}>Close project</Button>
</header>
