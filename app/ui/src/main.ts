import "./style.css";
import * as api from "./api";
import type { Cell, Impact, Matrix } from "./api";
import { CHANGED, DEFAULT_FILL, impactCellPaint } from "./cellstate";
import { reach } from "./graph";
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
const PITCH = 140;
const MARGIN_L = 110;
const MARGIN_T = 40;
const FWD = "#16a34a";
const BWD = "#2563eb";

function matrixView(m: Matrix) {
  let impact: Impact | null = null;
  let selected: string | null = null;
  let hovered: string | null = null;

  const cellById = new Map(m.cells.map((c) => [c.id, c]));
  const symName = new Map<string, string>();
  for (const c of m.cells) for (const sy of c.symbols) symName.set(sy.id, sy.name);
  const maxSize = Math.max(1, ...m.cells.map((c) => c.size));
  const col = (d: string) => m.domains.indexOf(d);
  const row = (l: Cell["layer"]) => m.layers.indexOf(l);
  const center = (c: Cell): [number, number] => [
    MARGIN_L + col(c.domain) * PITCH + PITCH / 2,
    MARGIN_T + row(c.layer) * PITCH + PITCH / 2,
  ];
  const sideOf = (c: Cell) => 20 + (PITCH - 50) * Math.sqrt(c.size / maxSize);

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
  const clearBtn = h("button", { textContent: "Clear", disabled: true });
  const msg = h("span", { className: "error" });
  const legend = h("span", { className: "legend muted" });
  legend.innerHTML =
    `<span style="background:${FWD}"></span>reaches (forward)` +
    `<span style="background:${BWD}"></span>reached by (backward)`;
  const toolbar = h("div", { className: "toolbar" }, baseIn, headIn, showBtn, clearBtn, msg, legend);
  // Analysis and impact warnings (e.g. indexing failed → tree-sitter only).
  const warnBox = h("div", { className: "warnings" });
  const showWarnings = (ws: string[]) => {
    warnBox.replaceChildren(...ws.map((w) => h("div", { textContent: `⚠ ${w}` })));
    warnBox.hidden = ws.length === 0;
  };
  showWarnings(m.warnings ?? []);

  // SVG
  const width = MARGIN_L + m.domains.length * PITCH + 20;
  const height = MARGIN_T + m.layers.length * PITCH + 20;
  const svg = s("svg", { viewBox: `0 0 ${width} ${height}` });
  const defs = s("defs");
  for (const [id, color] of [["a-def", "#999"], ["a-fwd", FWD], ["a-bwd", BWD], ["a-imp", CHANGED]]) {
    const mk = s("marker", { id, viewBox: "0 0 10 10", refX: 9, refY: 5, markerWidth: 6, markerHeight: 6, orient: "auto" });
    mk.append(s("path", { d: "M0,0 L10,5 L0,10 z", fill: color }));
    defs.append(mk);
  }
  svg.append(defs);

  m.domains.forEach((d, i) => {
    const t = s("text", { x: MARGIN_L + i * PITCH + PITCH / 2, y: 24, "text-anchor": "middle", "font-weight": "bold" });
    t.textContent = d;
    svg.append(t);
  });
  m.layers.forEach((l, i) => {
    const t = s("text", { x: 8, y: MARGIN_T + i * PITCH + PITCH / 2, "font-weight": "bold" });
    t.textContent = l;
    svg.append(t);
  });
  // Empty slots
  for (const l of m.layers)
    for (const d of m.domains) {
      if (cellById.has(`${l}:${d}`)) continue;
      svg.append(
        s("rect", {
          x: MARGIN_L + col(d) * PITCH + 20,
          y: MARGIN_T + row(l) * PITCH + 20,
          width: PITCH - 40,
          height: PITCH - 40,
          fill: "none",
          stroke: "#ddd",
          "stroke-dasharray": "3 3",
          opacity: 0.6,
        }),
      );
    }

  // Edges (under cells)
  const edgeEls: { e: Matrix["edges"][number]; el: SVGPathElement }[] = [];
  for (const e of m.edges) {
    const a = cellById.get(e.from);
    const b = cellById.get(e.to);
    if (!a || !b) continue;
    const [x1, y1] = center(a);
    const [x2, y2] = center(b);
    const dx = x2 - x1;
    const dy = y2 - y1;
    const len = Math.hypot(dx, dy) || 1;
    // Stop at the square border (approx.) so arrowheads stay visible.
    const trim = (c: Cell) => sideOf(c) / 2 + 2;
    const sx = x1 + (dx / len) * trim(a);
    const sy = y1 + (dy / len) * trim(a);
    const ex = x2 - (dx / len) * trim(b);
    const ey = y2 - (dy / len) * trim(b);
    const bend = 0.15 * len;
    const cx = (sx + ex) / 2 - (dy / len) * bend;
    const cy = (sy + ey) / 2 + (dx / len) * bend;
    const el = s("path", { d: `M${sx},${sy} Q${cx},${cy} ${ex},${ey}`, fill: "none" });
    el.style.pointerEvents = "none";
    svg.append(el);
    edgeEls.push({ e, el });
  }

  // Cells
  const cellEls = new Map<string, SVGRectElement>();
  for (const c of m.cells) {
    const [cx, cy] = center(c);
    const side = sideOf(c);
    const g = s("g");
    const r = s("rect", { x: cx - side / 2, y: cy - side / 2, width: side, height: side, rx: 3, stroke: "#333", "stroke-width": 1 });
    r.style.cursor = "pointer";
    const t = s("text", { x: cx, y: cy + side / 2 + 13, "text-anchor": "middle" });
    t.textContent = `${c.layer}/${c.domain} (${c.size})`;
    t.style.pointerEvents = "none";
    g.append(r, t);
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

  const panel = h("aside");
  const main = h("div", { className: "main" }, svg as unknown as Node, panel);
  show(header, toolbar, warnBox, main);

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
      r.setAttribute("stroke-width", id === selected ? "3" : "1");
    }
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

  showBtn.onclick = async () => {
    msg.textContent = "";
    showBtn.disabled = true;
    try {
      impact = await api.impact(projectPath, baseIn.value.trim() || null, headIn.value.trim() || null);
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
  };
  clearBtn.onclick = () => {
    impact = null;
    showWarnings(m.warnings ?? []);
    clearBtn.disabled = true;
    paint();
    renderPanel();
  };

  paint();
  renderPanel();
}

home();
