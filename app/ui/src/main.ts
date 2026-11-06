import "./style.css";
import * as api from "./api";
import type { Impact, Matrix } from "./api";
import { CHANGED, DEFAULT_FILL, impactCellPaint } from "./cellstate";
import { reach } from "./graph";
import { clickCommit, isSelected, relativeDate, selectionDiff } from "./history";
import type { Selection } from "./history";
import { rowSvg } from "./graphsvg";
import { layoutLanes } from "./lanes";
import { layout } from "./layout";
import type { Mode } from "./layout";
import { loadMode, saveMode } from "./mode";
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
  show(
    h(
      "div",
      { className: "center" },
      h("h2", { textContent: "Analyzing…" }),
      h("div", { className: "bar" }, fill),
      label,
    ),
  );
  try {
    const m = await analyzeAndRecord(projectPath, (path) =>
      api.analyze(path, (p) => {
        label.textContent = `${p.step} (${p.pct}%)`;
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

function matrixView(m: Matrix) {
  let impact: Impact | null = null;
  let selected: string | null = null;
  let hovered: string | null = null;

  const cellById = new Map(m.cells.map((c) => [c.id, c]));
  const symName = new Map<string, string>();
  for (const c of m.cells) for (const sy of c.symbols) symName.set(sy.id, sy.name);
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
  const toolbar = h("div", { className: "toolbar" }, advanced, denseBtn, clearBtn, msg, legend);
  // Analysis and impact warnings (e.g. indexing failed → tree-sitter only).
  const warnBox = h("div", { className: "warnings" });
  const showWarnings = (ws: string[]) => {
    warnBox.replaceChildren(...ws.map((w) => h("div", { textContent: `⚠ ${w}` })));
    warnBox.hidden = ws.length === 0;
  };
  showWarnings(m.warnings ?? []);

  // SVG
  const svg = s("svg");
  const cellEls = new Map<string, SVGRectElement>();
  let edgeEls: { e: Matrix["edges"][number]; el: SVGPathElement }[] = [];

  function build() {
    const lay = layout(mode, m.layers, m.domains, m.cells);
    svg.replaceChildren();
    cellEls.clear();
    edgeEls = [];
    denseBtn.textContent = mode === "dense" ? "Dense: on" : "Dense: off";
    svg.setAttribute("viewBox", `0 0 ${lay.width} ${lay.height}`);
    // Dense mode keeps its natural pixel size: stretching it would blow up the
    // headers and borders instead of showing more cells.
    svg.classList.toggle("dense", !lay.detailed);
    if (lay.detailed) {
      svg.removeAttribute("width");
      svg.removeAttribute("height");
    } else {
      svg.setAttribute("width", String(lay.width));
      svg.setAttribute("height", String(lay.height));
    }
    const defs = s("defs");
    for (const [id, color] of [["a-def", "#999"], ["a-fwd", FWD], ["a-bwd", BWD], ["a-imp", CHANGED]]) {
      const mk = s("marker", { id, viewBox: "0 0 10 10", refX: 9, refY: 5, markerWidth: 6, markerHeight: 6, orient: "auto" });
      mk.append(s("path", { d: "M0,0 L10,5 L0,10 z", fill: color }));
      defs.append(mk);
    }
    svg.append(defs);

    const fs: Record<string, number> = lay.detailed ? {} : { "font-size": 10 };
    for (const l of lay.domainLabels) {
      const t = s("text", { x: l.x, y: l.y, "font-weight": "bold", ...fs });
      if (lay.rotateDomains) {
        t.setAttribute("transform", `rotate(-90 ${l.x} ${l.y})`);
        // Centre the glyphs on the column, not their baseline.
        t.setAttribute("dominant-baseline", "middle");
      }
      else t.setAttribute("text-anchor", "middle");
      t.textContent = l.text;
      svg.append(t);
    }
    for (const l of lay.layerLabels) {
      const t = s("text", { x: l.x, y: l.y, "font-weight": "bold", ...fs });
      t.textContent = l.text;
      svg.append(t);
    }
    const tileById = new Map(lay.tiles.flatMap((t) => (t.id ? [[t.id, t] as const] : [])));
    // Empty slots
    for (const t of lay.tiles) {
      if (t.id) continue;
      svg.append(
        lay.detailed
          ? s("rect", { x: t.x, y: t.y, width: t.w, height: t.h, fill: "none", stroke: "#ddd", "stroke-dasharray": "3 3", opacity: 0.6 })
          : s("rect", { x: t.x + 0.5, y: t.y + 0.5, width: t.w - 1, height: t.h - 1, fill: "#eef0f3" }),
      );
    }

    // Edges (under cells), normal mode only
    for (const e of lay.detailed ? m.edges : []) {
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
      svg.append(el);
      edgeEls.push({ e, el });
    }

    // Cells
    for (const c of m.cells) {
      const t = tileById.get(c.id);
      if (!t) continue;
      const g = s("g");
      const r = lay.detailed
        ? s("rect", { x: t.x, y: t.y, width: t.w, height: t.h, rx: 3, stroke: "#333", "stroke-width": 1 })
        : s("rect", { x: t.x + 0.5, y: t.y + 0.5, width: t.w - 1, height: t.h - 1, stroke: "#333" });
      r.style.cursor = "pointer";
      const label = `${c.layer}/${c.domain} (${c.size})`;
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
      svg.append(g);
      cellEls.set(c.id, r);
    }
    paint();
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
  // The view scrolls when a dense grid is larger than the space left.
  const view = h("div", { className: "view" }, svg as unknown as Node);
  const main = h("div", { className: "main" }, sidebar, view, panel);
  show(header, toolbar, warnBox, main);
  build();

  function paint() {
    const impactById = new Map(impact?.impacted_cells.map((x) => [x.id, x.distance]));
    const changed = new Set(impact?.changed_cells ?? []);
    let fwd = new Set<string>();
    let bwd = new Set<string>();
    if (hovered) {
      fwd = reach(hovered, m.edges, "forward");
      bwd = reach(hovered, m.edges, "backward");
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
    if (selG && svg.lastChild !== (selG as Node)) svg.append(selG);
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
        h("h3", { textContent: `${c.layer}/${c.domain} — ${c.symbols.length} symbols` }),
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
  async function runImpact(base: string | null, head: string | null) {
    msg.textContent = "";
    showBtn.disabled = true;
    try {
      impact = await api.impact(projectPath, base, head);
      showWarnings(impact.warnings ?? m.warnings ?? []);
      clearBtn.disabled = false;
      hovered = null;
      paint();
      renderPanel();
    } catch (e) {
      msg.textContent = `Impact failed: ${String(e)}`;
    } finally {
      showBtn.disabled = false;
    }
  }
  clearBtn.onclick = () => {
    impact = null;
    histSel = null;
    renderHistory();
    showWarnings(m.warnings ?? []);
    clearBtn.disabled = true;
    paint();
    renderPanel();
  };

  paint();
  renderPanel();
}

home();
