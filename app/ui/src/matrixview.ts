import type { Impact, Matrix } from "./api";
import { aggregate, aggregateImpact, bands, columns, flatColumns, initialExpanded } from "./domaintree";
import type { ViewCell, ViewMatrix } from "./domaintree";
import { CHANGED, DEFAULT_FILL, impactCellPaint } from "./cellstate";
import { reach } from "./graph";
import { DENSE_STEP, PITCH, layout } from "./layout";
import type { Mode } from "./layout";
import { fit, oneToOne, panBy, resize as resizeVb, isFit, toUser, viewBoxAttr, zoomAtClamped } from "./viewport";
import type { ViewBox } from "./viewport";
import { s, setSvgAttr } from "./svgattr";

// The matrix SVG, still imperative (rendering moves to Svelte in #39). It is
// mounted into a container by MatrixView.svelte; app state flows in through the
// controller's setters and the selected cell flows out through `onSelect`.

export interface MatrixController {
  setMatrix(m: Matrix): void;
  setMode(mode: Mode): void;
  setImpact(impact: Impact | null): void;
  setSelected(id: string | null): void;
  zoomIn(): void;
  zoomOut(): void;
  fit(): void;
  destroy(): void;
}

const FWD = "var(--fwd)";
const BWD = "var(--bwd)";
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

export function createMatrixView(
  host: HTMLElement,
  init: { matrix: Matrix; mode: Mode; impact: Impact | null; selected: string | null },
  onSelect: (cell: ViewCell | null) => void,
): MatrixController {
  let m = init.matrix;
  let impact = init.impact;
  let selected = init.selected;
  let hovered: string | null = null;
  let mode = init.mode;
  const view = host;

  // Collapsible columns along the domain tree (module domains), else one per domain.
  const treeOf = (mx: Matrix) =>
    mx.domain_tree && mx.domain_tree.length > 0 ? mx.domain_tree : undefined;
  let tree = treeOf(m);
  let expanded = tree ? initialExpanded(tree) : new Set<string>();
  let vm: ViewMatrix = aggregate(m, flatColumns(m.domains));

  // SVG
  const svg = s("svg");
  view.append(svg);
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
    for (const [id, color] of [["a-def", "var(--edge)"], ["a-fwd", FWD], ["a-bwd", BWD], ["a-imp", CHANGED]]) {
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
      const r = s("rect", { x: x + 1, y: b.depth * bandH + 1, width: w - 2, height: bandH - 2, rx: 3, fill: "var(--band-fill)", stroke: "var(--band-stroke)" });
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
          ? s("rect", { x: t.x, y: t.y, width: t.w, height: t.h, fill: "none", stroke: "var(--border)", "stroke-dasharray": "3 3", opacity: 0.6 })
          : s("rect", { x: t.x + 0.5, y: t.y + 0.5, width: t.w - 1, height: t.h - 1, fill: "var(--tile)" }),
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
        ? s("rect", { x: t.x, y: t.y, width: t.w, height: t.h, rx: 3, stroke: "var(--cell-stroke)", "stroke-width": 1 })
        : s("rect", { x: t.x + 0.5, y: t.y + 0.5, width: t.w - 1, height: t.h - 1, stroke: "var(--cell-stroke)" });
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
        onSelect(c);
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
    onSelect(null);
    build(false);
  }
  // Keep the viewBox aspect equal to the pane's when the window or panels resize.
  const ro = new ResizeObserver(() => {
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
        if (id === hovered) fill = "var(--hover)";
        else if (fwd.has(id) && bwd.has(id)) fill = "var(--both)";
        else if (fwd.has(id)) fill = FWD;
        else if (bwd.has(id)) fill = BWD;
        else opacity = 0.25;
      } else if (impact) {
        ({ fill, opacity } = impactCellPaint(id, changed, impactById));
      }
      setSvgAttr(r, "fill", fill);
      r.setAttribute("opacity", String(opacity));
      const [base, sel] = mode === "dense" ? STROKE_DENSE : STROKE_NORMAL;
      r.setAttribute("stroke-width", String(id === selected ? sel : base));
    }
    // Draw the selected cell last so its thicker outline is not covered by
    // neighbours (tiles touch in dense mode).
    const selG = selected ? cellEls.get(selected)?.parentNode : null;
    if (selG && body.lastChild !== (selG as Node)) body.append(selG);
    for (const { e, el } of edgeEls) {
      let color = "var(--edge)";
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
      setSvgAttr(el, "stroke", color);
      el.setAttribute("stroke-width", String(Math.max(1, 1 + Math.log(e.weight))));
      el.setAttribute("opacity", String(opacity));
      el.setAttribute("marker-end", `url(#${marker})`);
    }
  }

  build();
  return {
    setMatrix(next) {
      if (next === m) return;
      m = next;
      // The domain tree can differ between commits: start from its default expansion.
      tree = treeOf(m);
      expanded = tree ? initialExpanded(tree) : new Set<string>();
      selected = null;
      hovered = null;
      build();
    },
    setMode(next) {
      if (next === mode) return;
      mode = next;
      hovered = null;
      build();
    },
    setImpact(next) {
      impact = next;
      hovered = null;
      paint();
    },
    setSelected(id) {
      if (id === selected) return;
      selected = id;
      paint();
    },
    zoomIn: () => zoomBy(1.5, paneW / 2, paneH / 2),
    zoomOut: () => zoomBy(1 / 1.5, paneW / 2, paneH / 2),
    fit() {
      measure();
      vb = fitVb = fit(content.w, content.h, paneW, paneH);
      applyView();
    },
    destroy() {
      ro.disconnect();
      svg.remove();
    },
  };
}
