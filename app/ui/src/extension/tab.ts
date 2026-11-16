// The "Structure" tab in GitHub's pull request tab bar, built like GitHub's own tabs
// (icon, label, active underline) whatever the bar's markup: the classic `TabNav` or the
// newer Primer `UnderlineNav` (Overview / Commits / Checks / Files changed).

const SVG_NS = "http://www.w3.org/2000/svg";
/** Octicon `table-16`: Structure shows a matrix. */
const ICON_PATH =
  "M0 1.75C0 .784.784 0 1.75 0h12.5C15.216 0 16 .784 16 1.75v12.5A1.75 1.75 0 0 1 14.25 16H1.75A1.75 1.75 0 0 1 0 14.25ZM6.5 6.5v8h7.75a.25.25 0 0 0 .25-.25V6.5Zm8-1.5V1.75a.25.25 0 0 0-.25-.25H6.5V5Zm-13 1.5v7.75c0 .138.112.25.25.25H5v-8ZM5 5V1.5H1.75a.25.25 0 0 0-.25.25V5Z";
/** Remembers, on the tab GitHub had selected, the selection classes moved to ours. */
const MOVED_ATTR = "data-structure-selected";
const PR_TABS = ["commits", "checks", "files", "changes"];

/** Path of a PR tab link (`/o/r/pull/1/commits`) -> its tab name, else null. */
export function prTabName(href: string, prPath: string): string | null {
  const path = href.replace(/^https:\/\/github\.com/, "").replace(/[?#].*$/, "");
  if (path === prPath) return "overview";
  if (!path.startsWith(`${prPath}/`)) return null;
  const rest = path.slice(prPath.length + 1);
  return PR_TABS.includes(rest) ? rest : null;
}

/** Class names that mark a tab as selected (`selected`, `selected__NLIm71`, …). */
export function selectionClasses(className: string): string[] {
  return className.split(/\s+/).filter((c) => /selected/i.test(c));
}

function tabLinks(root: ParentNode, prPath: string): HTMLAnchorElement[] {
  return [...root.querySelectorAll<HTMLAnchorElement>("a[href]")].filter(
    (a) => prTabName(a.getAttribute("href") ?? "", prPath) !== null,
  );
}

/** The PR tab bar: the innermost element holding at least two other PR tab links. */
export function findTabBar(doc: Document, prPath: string, fallback: string): HTMLElement | null {
  const known = doc.querySelector<HTMLElement>(fallback);
  if (known) return known;
  const links = tabLinks(doc, prPath).filter((a) => prTabName(a.getAttribute("href") ?? "", prPath) !== "overview");
  const first = links[0];
  if (!first) return null;
  for (let e = first.parentElement; e && e !== doc.body; e = e.parentElement) {
    if (tabLinks(e, prPath).length >= 2) return e.closest<HTMLElement>("nav") ?? e;
  }
  return null;
}

function icon(like: SVGElement | null): SVGElement {
  const svg = document.createElementNS(SVG_NS, "svg");
  for (const name of ["class", "style", "width", "height"]) {
    const v = like?.getAttribute(name);
    if (v) svg.setAttribute(name, v);
  }
  svg.setAttribute("class", (svg.getAttribute("class") ?? "octicon").replace(/octicon-[\w-]+/g, "") + " octicon-table");
  svg.setAttribute("viewBox", "0 0 16 16");
  svg.setAttribute("width", like?.getAttribute("width") ?? "16");
  svg.setAttribute("height", like?.getAttribute("height") ?? "16");
  svg.setAttribute("fill", "currentColor");
  svg.setAttribute("aria-hidden", "true");
  const path = document.createElementNS(SVG_NS, "path");
  path.setAttribute("d", ICON_PATH);
  svg.append(path);
  return svg;
}

/** Turns a clone of a GitHub tab link into the Structure tab: same look, own icon and label. */
function restyle(a: HTMLAnchorElement, id: string) {
  a.id = id;
  a.href = "#structure";
  a.removeAttribute("aria-current");
  a.classList.remove(...selectionClasses(a.className));
  for (const attr of [...a.attributes]) {
    if (attr.name.startsWith("data-") && attr.name !== "data-component") a.removeAttribute(attr.name);
  }
  // Counter and screen-reader count of the cloned tab mean nothing here.
  for (const e of a.querySelectorAll('[data-component="CounterLabel"], [class*="CounterLabel"], [class*="VisuallyHidden"]')) {
    e.remove();
  }
  const oldIcon = a.querySelector("svg");
  oldIcon?.replaceWith(icon(oldIcon));
  if (!oldIcon) a.prepend(icon(null));
  // Label: the longest text node (`Files changed`), wherever the markup nests it.
  const walker = document.createTreeWalker(a, NodeFilter.SHOW_TEXT);
  let label: Text | null = null;
  for (let n = walker.nextNode(); n; n = walker.nextNode()) {
    if (n.textContent?.trim() && (!label || n.textContent.trim().length > (label.textContent?.trim().length ?? 0))) {
      label = n as Text;
    }
  }
  if (label) label.textContent = "Structure";
  else a.append("Structure");
}

/** Adds the Structure tab after the last PR tab of `bar`; returns it (existing or new). */
export function ensureStructureTab(bar: HTMLElement, prPath: string, id: string): HTMLAnchorElement {
  const existing = document.getElementById(id);
  if (existing instanceof HTMLAnchorElement && bar.contains(existing)) return existing;
  existing?.remove();
  const links = tabLinks(bar, prPath);
  const template = links[links.length - 1];
  const tab = document.createElement("a");
  if (!template) {
    tab.id = id;
    tab.href = "#structure";
    tab.textContent = "Structure";
    bar.append(tab);
    return tab;
  }
  // Primer lists wrap each link in an <li>: clone the item so spacing and the underline match.
  const item = template.parentElement?.tagName === "LI" ? template.parentElement : null;
  if (item) {
    const li = item.cloneNode(true) as HTMLElement;
    const a = li.querySelector<HTMLAnchorElement>("a[href]");
    if (a) {
      restyle(a, id);
      item.after(li);
      return a;
    }
  }
  const a = template.cloneNode(true) as HTMLAnchorElement;
  restyle(a, id);
  template.after(a);
  return a;
}

/**
 * Shows `tab` as the selected one (underline, bold) or gives the selection back to the tab
 * GitHub had selected. Idempotent: called again after each GitHub re-render.
 */
export function selectStructureTab(bar: HTMLElement, tab: HTMLElement, selected: boolean) {
  if (selected) {
    for (const a of bar.querySelectorAll<HTMLElement>('a[aria-current]')) {
      if (a === tab) continue;
      const classes = selectionClasses(a.className);
      const before = a.getAttribute(MOVED_ATTR)?.split(" ").filter(Boolean) ?? [];
      a.setAttribute(MOVED_ATTR, [...new Set([...before, ...classes])].join(" "));
      a.classList.remove(...classes);
      a.removeAttribute("aria-current");
    }
    const moved = [...bar.querySelectorAll(`[${MOVED_ATTR}]`)].flatMap((e) =>
      (e.getAttribute(MOVED_ATTR) ?? "").split(" ").filter(Boolean),
    );
    tab.classList.add(...moved);
    tab.setAttribute("aria-current", "page");
  } else {
    tab.classList.remove(...selectionClasses(tab.className));
    tab.removeAttribute("aria-current");
    for (const a of bar.querySelectorAll<HTMLElement>(`[${MOVED_ATTR}]`)) {
      a.classList.add(...(a.getAttribute(MOVED_ATTR) ?? "").split(" ").filter(Boolean));
      a.setAttribute("aria-current", "page");
      a.removeAttribute(MOVED_ATTR);
    }
  }
}
