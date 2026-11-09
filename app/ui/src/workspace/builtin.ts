import DetailsPanel from "$lib/components/DetailsPanel.svelte";
import HistorySidebar from "$lib/components/HistorySidebar.svelte";
import MatrixPanel from "$lib/components/MatrixPanel.svelte";
import { app } from "../state.svelte";
import { panels } from "./registry";

// The panels Structure ships with. A new panel is one more `register` call
// (here or in its own module imported once at startup).
panels.register({ id: "matrix", title: "Matrix", component: MatrixPanel, available: () => app.matrix !== null });
panels.register({ id: "details", title: "Details", component: DetailsPanel });
panels.register({ id: "history", title: "History", component: HistorySidebar, available: () => app.history !== null });
