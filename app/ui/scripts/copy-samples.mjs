// Single source of truth: docs/samples -> public/samples (git-ignored copy).
import { cpSync, mkdirSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const dest = join(here, "..", "public", "samples");
mkdirSync(dest, { recursive: true });
cpSync(join(here, "..", "..", "..", "docs", "samples"), dest, { recursive: true });
