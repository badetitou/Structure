import type { Progress } from "../api";

/** Progress shown before the first report of a run (analysis or impact). */
export const START: Progress = { step: "starting…", pct: 0 };
