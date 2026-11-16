import type { Mode } from "../layout";
import {
  loadAligned,
  loadAngled,
  loadMode,
  loadShowHttp,
  loadShowRpc,
  loadTransposed,
  saveAligned,
  saveAngled,
  saveMode,
  saveShowHttp,
  saveShowRpc,
  saveTransposed,
} from "../mode";

/** How the matrix is drawn; each option is read from and saved to localStorage. */
export class ViewOptions {
  mode = $state<Mode>(loadMode());
  /** Draw front-to-back HTTP links. */
  showHttp = $state(loadShowHttp());
  /** Draw GWT RPC links. */
  showRpc = $state(loadShowRpc());
  /** Domains as rows and layers as columns. */
  transposed = $state(loadTransposed());
  /** Multi-project matrix: front and back domains share rows, projects get their own layer segment. */
  aligned = $state(loadAligned());
  /** Rotated top labels drawn at 45 degrees instead of upright. */
  angledLabels = $state(loadAngled());

  setMode(mode: Mode) {
    this.mode = mode;
    saveMode(mode);
  }
  setShowHttp(on: boolean) {
    this.showHttp = on;
    saveShowHttp(on);
  }
  setShowRpc(on: boolean) {
    this.showRpc = on;
    saveShowRpc(on);
  }
  setTransposed(on: boolean) {
    this.transposed = on;
    saveTransposed(on);
  }
  setAligned(on: boolean) {
    this.aligned = on;
    saveAligned(on);
  }
  setAngledLabels(on: boolean) {
    this.angledLabels = on;
    saveAngled(on);
  }
}

export const options = new ViewOptions();
