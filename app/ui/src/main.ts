import { RequestGuard, shownMatrix } from "./matrixsel";
import "./style.css";
import { COLLAPSED_WIDTH, clampWidth, loadPanels, savePanels } from "./panels";
import type { PanelsState } from "./panels";
import * as api from "./api";
import type { Impact, Matrix } from "./api";
import { aggregate, aggregateImpact, bands, columns, flatColumns, initialExpanded } from "./domaintree";
import type { ViewCell, ViewMatrix } from "./domaintree";
import { CHANGED, DEFAULT_FILL, impactCellPaint } from "./cellstate";
import { reach } from "./graph";
import { clickCommit, isSelected, relativeDate, selectionDiff } from "./history";
import type { Selection } from "./history";
import { rowSvg } from "./graphsvg";
import { layoutLanes } from "./lanes";
import { DENSE_STEP, PITCH, layout } from "./layout";
import type { Mode } from "./layout";
import { loadMode, saveMode } from "./mode";
import { fit, oneToOne, panBy, resize as resizeVb, isFit, toUser, viewBoxAttr, zoomAtClamped } from "./viewport";
import type { ViewBox } from "./viewport";
import { analyzeAndRecord, loadRecent, removeRecent, saveRecent } from "./recent";

const root = document.getElementById("app") as HTMLElement;
const SVG = "http://www.w3.org/2000/svg";

function h<K extends keyof HTMLElementTagNameMap>(
  tag: K,
  props: Partial<HTMLElementTagNameMap[K]> = {},
  ...children: (Node | string)[]
): HTMLElementTagNameMap[K] {
  const el = Object.assign(document.createElement(tag), props);
  el.append(...children);
  return el;
}
/** Drag handle + chevron on the inner edge of a side panel: resize, collapse (chevron or dblclick). */
function sideHandle(el: HTMLElement, side: "left" | "right", st: PanelsState): HTMLElement {
  const state = st[side];
  const other = st[side === "left" ? "right" : "left"];
  const otherWidth = () => (other.collapsed ? COLLAPSED_WIDTH : other.width);
  const fit = (w: number) => clampWidth(w, w, window.innerWidth, otherWidth());
  const chevron = h("button", { className: "chevron", type: "button" });
  const handle = h("div", { className: `handle ${side}` }, chevron);
  const apply = () => {
    state.width = fit(state.width);
    el.classList.toggle("collapsed", state.collapsed);
    el.style.width = `${state.collapsed ? COLLAPSED_WIDTH : state.width}px`;
    handle.classList.toggle("is-collapsed", state.collapsed);
    chevron.textContent = state.collapsed === (side === "left") ? "›" : "‹";
    chevron.title = state.collapsed ? "Expand panel" : "Collapse panel";
  };
  const toggle = () => {
    state.collapsed = !state.collapsed;
    apply();
    savePanels(st);
  };
  chevron.onclick = toggle;
  chevron.ondblclick = (e) => e.stopPropagation();
  handle.ondblclick = toggle;
  handle.onpointerdown = (e) => {
    if (e.target === chevron || state.collapsed || e.button !== 0) return;
    e.preventDefault();
    e.stopPropagation();
    handle.setPointerCapture(e.pointerId);
    const x0 = e.clientX;
    const w0 = state.width;
    document.body.classList.add("resizing");
    handle.onpointermove = (m) => {
      const dx = m.clientX - x0;
      state.width = fit(side === "left" ? w0 + dx : w0 - dx);
      apply();
    };
    handle.onpointerup = handle.onpointercancel = () => {
      handle.onpointermove = handle.onpointerup = handle.onpointercancel = null;
      document.body.classList.remove("resizing");
      savePanels(st);
    };
  };
  const onResize = () => {
    if (!handle.isConnected) window.removeEventListener("resize", onResize);
    else apply();
  };
  window.addEventListener("resize", onResize);
  apply();
  return handle;
}
function s<K extends keyof SVGElementTagNameMap>(
  tag: K,
  attrs: Record<string, string | number> = {},
): SVGElementTagNameMap[K] {
  const el = document.createElementNS(SVG, tag);
  for (const [k, v] of Object.entries(attrs)) el.setAttribute(k, String(v));
  return el;
}
function show(...nodes: Node[]) {
  root.replaceChildren(...nodes);
}

let projectPath = "";

// ---- 1. Home ----
function home(error?: string) {
  const pathLabel = h("div", { className: "muted" }, projectPath || "No project selected");
  const hint = h("div", { className: "muted" }, "Supported: Java and TypeScript projects.");
  const analyzeBtn = h("button", { textContent: "Analyze", disabled: !projectPath });
  const openBtn = h("button", { textContent: "Open project…" });
  openBtn.onclick = async () => {
    const p = await api.pickFolder();
    if (p) {
      projectPath = p;
      home();
    }
  };
  analyzeBtn.onclick = () => {
    analyzeBtn.disabled = true;
    void runAnalysis();
  };
  const recentBox = h("div", { className: "recent" });
  const recents = loadRecent();
  if (recents.length) {
    recentBox.append(h("h3", { textContent: "Recent projects" }));
    for (const r of recents) {
      const open = h(
        "button",
        { className: "recent-open", title: r.path },
        h("strong", { textContent: r.name }),
        h("span", { className: "muted recent-path", textContent: r.path }),
        h("span", { className: "muted", textContent: new Date(r.lastOpened).toLocaleString() }),
      );
      open.onclick = () => {
        projectPath = r.path;
        void runAnalysis();
      };
      const del = h("button", {
        className: "recent-del",
        textContent: "×",
        title: "Remove from recent projects",
      });
      del.onclick = () => {
        saveRecent(removeRecent(loadRecent(), r.path));
        home(error);
      };
      recentBox.append(h("div", { className: "recent-row" }, open, del));
    }
  }
  show(
    h(
      "div",
      { className: "center" },
      h(
        "div",
        { className: "brand" },
        h("img", { src: "/logo.svg", alt: "", className: "logo-lg" }),
        h("h1", { textContent: "Structure" }),
      ),
      openBtn,
      pathLabel,
      hint,
      analyzeBtn,
      h("div", { className: "error", textContent: error ?? "" }),
      recentBox,
    ),
  );
}

// ---- 2. Progress ----
async function runAnalysis() {
  const label = h("div", { textContent: "starting…" });
  const fill = h("div");
  fill.style.width = "0%";
  const detail = h("div", { className: "muted small" });
  show(
    h(
      "div",
      { className: "center" },
      h("h2", { textContent: "Analyzing…" }),
      h("div", { className: "bar" }, fill),
      label,
      detail,
    ),
  );
  try {
    const m = await analyzeAndRecord(projectPath, (path) =>
      api.analyze(path, (p) => {
        label.textContent = `${p.step} (${p.pct}%)`;
        detail.textContent = p.detail ?? "";
        fill.style.width = `${p.pct}%`;
      }),
    );
    matrixView(m);
  } catch (e) {
    home(`Analysis failed: ${String(e)}`);
  }
}

// ---- 3. Matrix view ----
const FWD = "#16a34a";
const BWD = "#2563eb";
/** Cell stroke width [unselected, selected] per mode. */
const STROKE_NORMAL = [1, 3] as const;
const STROKE_DENSE = [0.5, 2] as const;
/** Height of one row of group bands above the column headers, per mode. */
const BAND_NORMAL = 22;
const BAND_DENSE = 14;

/** `text` cut with an ellipsis to fit `width` px at about `charW` px per glyph. */
function fitText(text: string, width: number, charW: number): string {
  const n = Math.floor(width / charW);
  if (text.length <= n) return text;
  return n < 2 ? "" : `${text.slice(0, n - 1)}\u2026`;
}

function matrixView(workMatrix: Matrix) {
  let m = workMatrix;
  let impact: Impact | null = null;
  let selected: string | null = null;
  let hovered: string | null = null;

  // Collapsible columns along the domain tree (module domains), else one per domain.
  const treeOf = (mx: Matrix) =>
    mx.domain_tree && mx.domain_tree.length > 0 ? mx.domain_tree : undefined;
  let tree = treeOf(m);
  let expanded = tree ? initialExpanded(tree) : new Set<string>();
  let vm: ViewMatrix = aggregate(m, flatColumns(m.domains));
  // Rebuilt from `vm` by build().
  let cellById = new Map<string, ViewCell>();
  let symName = new Map<string, string>();
  const indexSymbols = () => {
    symName = new Map<string, string>();
    for (const c of m.cells) for (const sy of c.symbols) symName.set(sy.id, sy.name);
  };
  indexSymbols();
  /** Show the matrix `impact` carries (snapshot) or the working-tree one. */
  function showMatrixFor(imp: Impact | null) {
    const next = shownMatrix(workMatrix, imp);
    if (next === m) return;
    m = next;
    // The domain tree can differ between commits: start from its default expansion.
    tree = treeOf(m);
    expanded = tree ? initialExpanded(tree) : new Set<string>();
    indexSymbols();
    selected = null;
    build();
  }
  let mode: Mode = loadMode();

  // Header
  const st = m.stats;
  const header = h(
    "header",
    {},
    h("img", { src: "/logo.svg", alt: "Structure", className: "logo-sm" }),
    h("h1", { textContent: m.project }),
    h("span", {
      className: "muted",
      textContent: `${m.language} · ${st.files} files · ${st.symbols} symbols · ${st.edges} edges · ${st.elapsed_ms} ms`,
    }),
  );
  const back = h("button", { textContent: "Close project" });
  back.onclick = () => home();
  header.append(back);

  // Toolbar
  const baseIn = h("input", { placeholder: "base (empty = HEAD)" });
  const headIn = h("input", { placeholder: "head (empty = working tree)" });
  const showBtn = h("button", { textContent: "Show impact" });
  const denseBtn = h("button", { className: "toggle" });
  const clearBtn = h("button", { textContent: "Clear", disabled: true });
  const msg = h("span", { className: "error" });
  const legend = h("span", { className: "legend muted" });
  legend.innerHTML =
    `<span style="background:${FWD}"></span>reaches (forward)` +
    `<span style="background:${BWD}"></span>reached by (backward)`;
  const advanced = h(
    "details",
    { className: "advanced" },
    h("summary", { textContent: "Advanced" }),
    baseIn,
    headIn,
    showBtn,
  );
  const zoomInBtn = h("button", { textContent: "+", title: "Zoom in" });
  const zoomOutBtn = h("button", { textContent: "\u2212", title: "Zoom out" });
  const fitBtn = h("button", { textContent: "Fit", title: "Show the whole matrix" });
  zoomInBtn.onclick = () => zoomBy(1.5, paneW / 2, paneH / 2);
  zoomOutBtn.onclick = () => zoomBy(1 / 1.5, paneW / 2, paneH / 2);
  fitBtn.onclick = () => {
    measure();
    vb = fitVb = fit(content.w, content.h, paneW, paneH);
    applyView();
  };
  const toolbar = h("div", { className: "toolbar" }, advanced, zoomOutBtn, zoomInBtn, fitBtn, denseBtn, clearBtn, msg, legend);
  // Analysis and impact warnings (e.g. indexing failed → tree-sitter only).
  const warnBox = h("div", { className: "warnings" });
  const showWarnings = (ws: string[]) => {
    warnBox.replaceChildren(...ws.map((w) => h("div", { textContent: `⚠ ${w}` })));
    warnBox.hidden = ws.length === 0;
  };
  showWarnings(m.warnings ?? []);

  // SVG
  const svg = s("svg");
  // Matrix content, shifted down below the group bands.
  const body = s("g");
  const cellEls = new Map<string, SVGRectElement>();
  // Zoom / pan state. The viewBox keeps the pane's aspect (see viewport.ts).
  let content = { w: 1, h: 1 };
  let dense1to1 = false;
  let vb: ViewBox = { x: 0, y: 0, w: 1, h: 1 };
  let fitVb: ViewBox = vb;
  let paneW = 1;
  let paneH = 1;
  const measure = () => {
    paneW = Math.max(view.clientWidth, 1);
    paneH = Math.max(view.clientHeight, 1);
  };
  const applyView = () => svg.setAttribute("viewBox", viewBoxAttr(vb));
  function resetView() {
    measure();
    fitVb = fit(content.w, content.h, paneW, paneH);
    // Dense mode starts at 1:1 pixels anchored top-left; Fit shows everything.
    vb = dense1to1 ? oneToOne(paneW, paneH) : fitVb;
    applyView();
  }
  function zoomBy(factor: number, sx: number, sy: number) {
    const p = toUser(vb, sx, sy, paneW, paneH);
    vb = zoomAtClamped(vb, factor, p.x, p.y, fitVb);
    applyView();
  }
  let edgeEls: { e: Matrix["edges"][number]; el: SVGPathElement }[] = [];

  function build(resetZoom = true) {
    vm = aggregate(m, tree ? columns(tree, expanded) : flatColumns(m.domains));
    cellById = new Map(vm.cells.map((c) => [c.id, c]));
    const cols = vm.columns;
    const lay = layout(
      mode,
      m.layers,
      cols.map((c) => c.key),
      vm.cells,
      cols.map((c) => (c.group ? `\u25b8 ${c.label}` : c.label)),
    );
    const bs = tree ? bands(tree, expanded) : [];
    const bandH = lay.detailed ? BAND_NORMAL : BAND_DENSE;
    const top = bs.length ? (Math.max(...bs.map((b) => b.depth)) + 1) * bandH : 0;
    const width = lay.width;
    const height = lay.height + top;
    svg.replaceChildren();
    body.replaceChildren();
    body.setAttribute("transform", `translate(0 ${top})`);
    cellEls.clear();
    edgeEls = [];
    denseBtn.textContent = mode === "dense" ? "Dense: on" : "Dense: off";
    // The viewBox is driven by zoom/pan. A mode switch or a new matrix resets
    // it; expand/collapse keeps the current view.
    content = { w: width, h: height };
    dense1to1 = !lay.detailed;
    if (resetZoom) resetView();
    else {
      measure();
      fitVb = fit(content.w, content.h, paneW, paneH);
      applyView();
    }
    const defs = s("defs");
    for (const [id, color] of [["a-def", "#999"], ["a-fwd", FWD], ["a-bwd", BWD], ["a-imp", CHANGED]]) {
      const mk = s("marker", { id, viewBox: "0 0 10 10", refX: 9, refY: 5, markerWidth: 6, markerHeight: 6, orient: "auto" });
      mk.append(s("path", { d: "M0,0 L10,5 L0,10 z", fill: color }));
      defs.append(mk);
    }
    svg.append(defs, body);

    const fs: Record<string, number> = lay.detailed ? {} : { "font-size": 10 };
    // Group bands: click collapses the group back into one column.
    const pitch = lay.detailed ? PITCH : DENSE_STEP;
    for (const b of bs) {
      const l0 = lay.domainLabels[b.start];
      if (!l0) continue;
      const x = l0.x - pitch / 2;
      const w = b.span * pitch;
      const g = s("g");
      g.style.cursor = "pointer";
      const r = s("rect", { x: x + 1, y: b.depth * bandH + 1, width: w - 2, height: bandH - 2, rx: 3, fill: "#e2e8f0", stroke: "#94a3b8" });
      const t = s("text", { x: x + w / 2, y: b.depth * bandH + bandH / 2 + 4, "text-anchor": "middle", "font-weight": "bold", ...fs });
      t.textContent = fitText(`\u25be ${b.label}`, w - 6, lay.detailed ? 8 : 6);
      t.style.pointerEvents = "none";
      const tip = s("title");
      tip.textContent = `Collapse ${b.label}`;
      g.append(r, t, tip);
      g.addEventListener("click", () => {
        expanded = new Set(expanded);
        expanded.delete(b.key);
        rebuild();
      });
      svg.append(g);
    }
    lay.domainLabels.forEach((l, i) => {
      const col = cols[i];
      const t = s("text", { x: l.x, y: l.y, "font-weight": "bold", ...fs });
      if (col?.group) {
        t.style.cursor = "pointer";
        t.addEventListener("click", () => {
          expanded = new Set(expanded).add(col.key);
          rebuild();
        });
      }
      if (lay.rotateDomains) {
        t.setAttribute("transform", `rotate(-90 ${l.x} ${l.y})`);
        // Centre the glyphs on the column, not their baseline.
        t.setAttribute("dominant-baseline", "middle");
      }
      else t.setAttribute("text-anchor", "middle");
      t.textContent = l.text;
      body.append(t);
    });
    for (const l of lay.layerLabels) {
      const t = s("text", { x: l.x, y: l.y, "font-weight": "bold", ...fs });
      t.textContent = l.text;
      body.append(t);
    }
    const tileById = new Map(lay.tiles.flatMap((t) => (t.id ? [[t.id, t] as const] : [])));
    // Empty slots
    for (const t of lay.tiles) {
      if (t.id) continue;
      body.append(
        lay.detailed
          ? s("rect", { x: t.x, y: t.y, width: t.w, height: t.h, fill: "none", stroke: "#ddd", "stroke-dasharray": "3 3", opacity: 0.6 })
          : s("rect", { x: t.x + 0.5, y: t.y + 0.5, width: t.w - 1, height: t.h - 1, fill: "#eef0f3" }),
      );
    }

    // Edges (under cells), normal mode only
    for (const e of lay.detailed ? vm.edges : []) {
      const a = tileById.get(e.from);
      const b = tileById.get(e.to);
      if (!a || !b) continue;
      const x1 = a.x + a.w / 2;
      const y1 = a.y + a.h / 2;
      const x2 = b.x + b.w / 2;
      const y2 = b.y + b.h / 2;
      const dx = x2 - x1;
      const dy = y2 - y1;
      const len = Math.hypot(dx, dy) || 1;
      // Stop at the square border (approx.) so arrowheads stay visible.
      const sx = x1 + (dx / len) * (a.w / 2 + 2);
      const sy = y1 + (dy / len) * (a.w / 2 + 2);
      const ex = x2 - (dx / len) * (b.w / 2 + 2);
      const ey = y2 - (dy / len) * (b.w / 2 + 2);
      const bend = 0.15 * len;
      const cx = (sx + ex) / 2 - (dy / len) * bend;
      const cy = (sy + ey) / 2 + (dx / len) * bend;
      const el = s("path", { d: `M${sx},${sy} Q${cx},${cy} ${ex},${ey}`, fill: "none" });
      el.style.pointerEvents = "none";
      body.append(el);
      edgeEls.push({ e, el });
    }

    // Cells
    for (const c of vm.cells) {
      const t = tileById.get(c.id);
      if (!t) continue;
      const g = s("g");
      const r = lay.detailed
        ? s("rect", { x: t.x, y: t.y, width: t.w, height: t.h, rx: 3, stroke: "#333", "stroke-width": 1 })
        : s("rect", { x: t.x + 0.5, y: t.y + 0.5, width: t.w - 1, height: t.h - 1, stroke: "#333" });
      r.style.cursor = "pointer";
      const label = `${c.layer}/${c.label} (${c.size})`;
      if (lay.detailed) {
        const tx = s("text", { x: t.x + t.w / 2, y: t.y + t.h + 13, "text-anchor": "middle" });
        tx.textContent = label;
        tx.style.pointerEvents = "none";
        g.append(r, tx);
      } else {
        g.append(r);
      }
      const tip = s("title");
      tip.textContent = label;
      r.append(tip);
      r.addEventListener("mouseenter", () => {
        hovered = c.id;
        paint();
      });
      r.addEventListener("mouseleave", () => {
        hovered = null;
        paint();
      });
      r.addEventListener("click", () => {
        selected = c.id;
        paint();
        renderPanel();
      });
      body.append(g);
      cellEls.set(c.id, r);
    }
    paint();
  }
  // Expand/collapse changes the cell ids: drop the selection and hover.
  function rebuild() {
    selected = null;
    hovered = null;
    build(false);
    renderPanel();
  }
  denseBtn.onclick = () => {
    mode = mode === "dense" ? "normal" : "dense";
    saveMode(mode);
    hovered = null;
    build();
  };

  const panel = h("aside");
  const sidebar = h("nav", { className: "history" });
  sidebar.hidden = true;
  const view = h("div", { className: "view" }, svg as unknown as Node);
  const panels = loadPanels();
  const main = h(
    "div",
    { className: "main" },
    sidebar,
    sideHandle(sidebar, "left", panels),
    view,
    sideHandle(panel, "right", panels),
    panel,
  );
  show(header, toolbar, warnBox, main);
  build();

  // Keep the viewBox aspect equal to the pane's when the window or panels resize.
  const ro = new ResizeObserver(() => {
    if (!view.isConnected) {
      ro.disconnect();
      return;
    }
    if (view.clientWidth === 0 || view.clientHeight === 0) return;
    const oldW = paneW;
    const wasFit = isFit(vb, fitVb);
    measure();
    fitVb = fit(content.w, content.h, paneW, paneH);
    vb = wasFit ? fitVb : resizeVb(vb, oldW, paneW, paneH);
    applyView();
  });
  ro.observe(view);
  // Wheel: pinch (ctrlKey) zooms around the cursor, two-finger scroll pans.
  svg.addEventListener(
    "wheel",
    (ev) => {
      ev.preventDefault();
      const unit = ev.deltaMode === 1 ? 16 : 1;
      if (ev.ctrlKey) {
        const r = svg.getBoundingClientRect();
        zoomBy(Math.exp(-ev.deltaY * unit * 0.01), ev.clientX - r.left, ev.clientY - r.top);
      } else {
        vb = panBy(vb, -ev.deltaX * unit, -ev.deltaY * unit, paneW, paneH);
        applyView();
      }
    },
    { passive: false },
  );
  // Drag to pan. Pointer capture only starts past a few px so plain clicks
  // still reach the cells; a real drag swallows the click that follows it.
  const DRAG_PX = 4;
  let drag: { id: number; x: number; y: number; moved: boolean } | null = null;
  let suppressClick = false;
  svg.addEventListener("pointerdown", (ev) => {
    if (ev.button !== 0) return;
    drag = { id: ev.pointerId, x: ev.clientX, y: ev.clientY, moved: false };
    suppressClick = false;
  });
  svg.addEventListener("pointermove", (ev) => {
    if (!drag || ev.pointerId !== drag.id) return;
    if (ev.buttons === 0) {
      // Released outside the svg before capture: the drag is stale.
      drag = null;
      svg.classList.remove("panning");
      return;
    }
    if (!drag.moved) {
      if (Math.hypot(ev.clientX - drag.x, ev.clientY - drag.y) < DRAG_PX) return;
      drag.moved = true;
      suppressClick = true;
      svg.setPointerCapture(ev.pointerId);
      svg.classList.add("panning");
    }
    vb = panBy(vb, ev.clientX - drag.x, ev.clientY - drag.y, paneW, paneH);
    drag.x = ev.clientX;
    drag.y = ev.clientY;
    applyView();
  });
  const endDrag = (ev: PointerEvent) => {
    if (!drag || ev.pointerId !== drag.id) return;
    if (drag.moved && svg.hasPointerCapture(ev.pointerId)) svg.releasePointerCapture(ev.pointerId);
    drag = null;
    svg.classList.remove("panning");
  };
  svg.addEventListener("pointerup", endDrag);
  svg.addEventListener("pointercancel", endDrag);
  svg.addEventListener(
    "click",
    (ev) => {
      if (suppressClick) {
        ev.stopPropagation();
        suppressClick = false;
      }
    },
    true,
  );

  function paint() {
    const { changed, impacted: impactById } = aggregateImpact(impact, vm.cellMap);
    let fwd = new Set<string>();
    let bwd = new Set<string>();
    if (hovered) {
      fwd = reach(hovered, vm.edges, "forward");
      bwd = reach(hovered, vm.edges, "backward");
    }
    for (const [id, r] of cellEls) {
      let fill = DEFAULT_FILL;
      let opacity = 1;
      if (hovered) {
        if (id === hovered) fill = "#facc15";
        else if (fwd.has(id) && bwd.has(id)) fill = "#a855f7";
        else if (fwd.has(id)) fill = FWD;
        else if (bwd.has(id)) fill = BWD;
        else opacity = 0.25;
      } else if (impact) {
        ({ fill, opacity } = impactCellPaint(id, changed, impactById));
      }
      r.setAttribute("fill", fill);
      r.setAttribute("opacity", String(opacity));
      const [base, sel] = mode === "dense" ? STROKE_DENSE : STROKE_NORMAL;
      r.setAttribute("stroke-width", String(id === selected ? sel : base));
    }
    // Draw the selected cell last so its thicker outline is not covered by
    // neighbours (tiles touch in dense mode).
    const selG = selected ? cellEls.get(selected)?.parentNode : null;
    if (selG && body.lastChild !== (selG as Node)) body.append(selG);
    for (const { e, el } of edgeEls) {
      let color = "#999";
      let marker = "a-def";
      let opacity = 0.5;
      if (hovered) {
        const inF = (id: string) => id === hovered || fwd.has(id);
        const inB = (id: string) => id === hovered || bwd.has(id);
        if (inF(e.from) && fwd.has(e.to)) {
          color = FWD;
          marker = "a-fwd";
          opacity = 0.95;
        } else if (inB(e.to) && bwd.has(e.from)) {
          color = BWD;
          marker = "a-bwd";
          opacity = 0.95;
        } else opacity = 0.1;
      } else if (impact) {
        const on = (id: string) => changed.has(id) || impactById.has(id);
        if (on(e.from) && on(e.to)) {
          color = CHANGED;
          marker = "a-imp";
          opacity = 0.9;
        } else opacity = 0.12;
      }
      el.setAttribute("stroke", color);
      el.setAttribute("stroke-width", String(Math.max(1, 1 + Math.log(e.weight))));
      el.setAttribute("opacity", String(opacity));
      el.setAttribute("marker-end", `url(#${marker})`);
    }
  }

  function symbolList(items: { name: string; sub: string }[]): HTMLElement {
    if (items.length === 0) return h("div", { className: "muted", textContent: "none" });
    return h("ul", {}, ...items.map((i) => h("li", {}, i.name, h("small", { textContent: i.sub }))));
  }

  function renderPanel() {
    const nodes: Node[] = [];
    if (impact) {
      nodes.push(
        h("h3", { textContent: `Changed symbols (${impact.changed_symbols.length})` }),
        symbolList(impact.changed_symbols.map((id) => ({ name: symName.get(id) ?? id, sub: id }))),
        h("h3", { textContent: `Impacted symbols (${impact.impacted_symbols.length})` }),
        symbolList(
          [...impact.impacted_symbols]
            .sort((a, b) => a.distance - b.distance)
            .map((x) => ({ name: symName.get(x.id) ?? x.id, sub: `distance ${x.distance} · ${x.id}` })),
        ),
      );
    }
    const c = selected ? cellById.get(selected) : undefined;
    if (c) {
      nodes.push(
        h("h3", { textContent: `${c.layer}/${c.label} — ${c.symbols.length} symbols` }),
        symbolList(c.symbols.map((x) => ({ name: x.name, sub: x.file }))),
      );
    } else if (!impact) {
      nodes.push(h("div", { className: "muted", textContent: "Click a cell to list its symbols." }));
    }
    panel.replaceChildren(...nodes);
  }

  // History sidebar: hidden when unavailable (not a git repo, error).
  let hist: api.History | null = null;
  let histSel: Selection | null = null;
  const pick = (sel: Selection) => {
    if (!hist) return;
    const d = selectionDiff(hist.commits, sel);
    if (!d) return;
    histSel = sel;
    renderHistory();
    void runImpact(d.base, d.head);
  };
  function renderHistory() {
    if (!hist) return;
    const rows: Node[] = [];
    const layout = layoutLanes(hist.commits);
    const headColor = layout.rows[0]?.color;
    if (hist.dirty) {
      const r = h(
        "div",
        { className: "commit" + (histSel?.kind === "worktree" ? " sel" : "") },
        rowSvg(layout.rows[0] ? { ...layout.rows[0], top: [], through: [], bottom: [] } : null, layout.width, {
          hollow: true,
          lineBelow: headColor,
        }),
        h(
          "div",
          { className: "ctext" },
          h("div", { className: "subj", textContent: "Working tree" }),
          h("div", { className: "meta", textContent: "uncommitted changes" }),
        ),
      );
      r.onclick = () => pick({ kind: "worktree" });
      rows.push(r);
    }
    hist.commits.forEach((c, i) => {
      const subj = h("div", { className: "subj" }, h("code", { textContent: c.short }), ` ${c.subject}`);
      subj.title = c.subject;
      const meta = h("div", { className: "meta" }, `${c.author} · ${relativeDate(c.date)}`);
      for (const ref of c.refs) meta.append(h("span", { className: "badge", textContent: ref }));
      const g = rowSvg(layout.rows[i], layout.width, {
        lineAbove: i === 0 && hist?.dirty ? headColor : undefined,
      });
      const r = h(
        "div",
        { className: "commit" + (isSelected(histSel, i) ? " sel" : "") },
        g,
        h("div", { className: "ctext" }, subj, meta),
      );
      r.onclick = (ev) => pick(clickCommit(histSel, i, ev.shiftKey));
      rows.push(r);
    });
    sidebar.replaceChildren(h("h3", { textContent: "History" }), ...rows);
    sidebar.hidden = false;
  }
  api
    .history(projectPath)
    .then((r) => {
      hist = r;
      renderHistory();
    })
    .catch(() => {
      sidebar.hidden = true;
    });

  showBtn.onclick = () => {
    // A typed diff is not a sidebar selection.
    histSel = null;
    renderHistory();
    void runImpact(baseIn.value.trim() || null, headIn.value.trim() || null);
  };
  const guard = new RequestGuard();
  async function runImpact(base: string | null, head: string | null) {
    msg.textContent = "";
    showBtn.disabled = true;
    const req = guard.begin();
    try {
      const res = await api.impact(projectPath, base, head);
      if (!guard.isCurrent(req)) return; // superseded or cleared meanwhile
      impact = res;
      showMatrixFor(impact);
      showWarnings(impact.warnings ?? m.warnings ?? []);
      clearBtn.disabled = false;
      hovered = null;
      paint();
      renderPanel();
    } catch (e) {
      if (guard.isCurrent(req)) msg.textContent = `Impact failed: ${String(e)}`;
    } finally {
      showBtn.disabled = false;
    }
  }
  clearBtn.onclick = () => {
    guard.invalidate();
    impact = null;
    histSel = null;
    renderHistory();
    showMatrixFor(null);
    showWarnings(m.warnings ?? []);
    clearBtn.disabled = true;
    paint();
    renderPanel();
  };

  paint();
  renderPanel();
}

home();
