// Content script (bundled as one IIFE): adds a "Structure" tab to GitHub pull request pages.
import { parsePrPath, prParam } from "./github";

const TAB_ID = "structure-pr-tab";
const BUTTON_ID = "structure-pr-button";
const NAV_SELECTOR = 'nav[aria-label="Pull request navigation"]';
const HIDDEN_ATTR = "data-structure-hidden";
const MIN_HEIGHT_ATTR = "data-structure-min-height";

interface PanelState {
  key: string;
  panel: HTMLElement;
  iframe: HTMLIFrameElement | null;
  open: boolean;
}

let state: PanelState | null = null;

function viewerUrl(key: string): string {
  return `${chrome.runtime.getURL("viewer.html")}?pr=${key}`;
}

function makePanel(key: string): HTMLElement {
  const panel = document.createElement("div");
  panel.style.cssText = "display:flex;flex-direction:column;width:100%;box-sizing:border-box;";
  const bar = document.createElement("div");
  bar.style.cssText = "padding:4px 8px;font-size:12px;text-align:right;";
  const link = document.createElement("a");
  link.href = viewerUrl(key);
  link.target = "_blank";
  link.rel = "noopener";
  link.textContent = "Open in a new tab";
  bar.append(link);
  panel.append(bar);
  return panel;
}

function ensureIframe(s: PanelState) {
  if (s.iframe) return;
  const iframe = document.createElement("iframe");
  iframe.src = viewerUrl(s.key);
  iframe.title = "Structure";
  iframe.style.cssText = "flex:1;width:100%;border:0;min-height:0;";
  s.panel.append(iframe);
  s.iframe = iframe;
}

/** Where GitHub renders the page of a repository; the panel goes at its end, outside React's tree. */
const CONTAINER_SELECTOR = "#repo-content-pjax-container, main";

/** The PR's own content: everything that follows the tab bar inside the content container. */
function prContent(
  nav: HTMLElement,
): { container: HTMLElement; ancestors: HTMLElement[]; following: HTMLElement[] } | null {
  const container = nav.closest<HTMLElement>(CONTAINER_SELECTOR);
  if (!container) return null;
  const ancestors: HTMLElement[] = [];
  const following: HTMLElement[] = [];
  for (let a: HTMLElement | null = nav; a && a !== container; a = a.parentElement) {
    ancestors.push(a);
    for (let e = a.nextElementSibling; e; e = e.nextElementSibling) {
      if (e instanceof HTMLElement && e !== state?.panel && e.tagName !== "SCRIPT" && e.tagName !== "TEMPLATE") {
        following.push(e);
      }
    }
  }
  return { container, ancestors, following };
}

function showContent() {
  for (const e of document.querySelectorAll<HTMLElement>(`[${HIDDEN_ATTR}]`)) {
    e.style.display = e.getAttribute(HIDDEN_ATTR) ?? "";
    e.removeAttribute(HIDDEN_ATTR);
  }
  for (const e of document.querySelectorAll<HTMLElement>(`[${MIN_HEIGHT_ATTR}]`)) {
    e.style.minHeight = e.getAttribute(MIN_HEIGHT_ATTR) ?? "";
    e.removeAttribute(MIN_HEIGHT_ATTR);
  }
}

function applyOpen(s: PanelState) {
  const nav = document.querySelector<HTMLElement>(NAV_SELECTOR);
  const content = nav ? prContent(nav) : null;
  if (content) {
    for (const e of content.following) {
      if (e.hasAttribute(HIDDEN_ATTR)) continue;
      e.setAttribute(HIDDEN_ATTR, e.style.display);
      e.style.display = "none";
    }
    // A min-height on the tab bar's ancestors (GitHub's app root has one) would leave a gap.
    for (const a of content.ancestors) {
      if (a.hasAttribute(MIN_HEIGHT_ATTR) || !(parseFloat(getComputedStyle(a).minHeight) > 0)) continue;
      a.setAttribute(MIN_HEIGHT_ATTR, a.style.minHeight);
      a.style.minHeight = "0";
    }
    // Last child of the container: with the PR content hidden it sits right under the tab bar.
    if (s.panel.parentElement !== content.container || s.panel.nextElementSibling) content.container.append(s.panel);
    s.panel.style.cssText =
      `display:flex;flex-direction:column;width:100%;box-sizing:border-box;height:${Math.max(400, window.innerHeight - (nav?.offsetHeight ?? 0) - 8)}px;`;
  } else {
    // No tab bar: cover the page.
    s.panel.style.cssText =
      "display:flex;flex-direction:column;position:fixed;inset:0;z-index:2147483000;background:Canvas;color:CanvasText;";
    if (!s.panel.isConnected) document.body.append(s.panel);
  }
  ensureIframe(s);
}

function setOpen(open: boolean) {
  if (!state) return;
  state.open = open;
  if (open) {
    applyOpen(state);
    // Tab bar at the top of the viewport, the panel filling the rest.
    (document.querySelector(NAV_SELECTOR) ?? state.panel).scrollIntoView({ block: "start" });
  } else {
    showContent();
    state.panel.style.display = "none";
  }
  const tab = document.getElementById(TAB_ID);
  if (tab) {
    if (open) tab.setAttribute("aria-current", "page");
    else tab.removeAttribute("aria-current");
  }
}

function toggle() {
  setOpen(!(state?.open ?? false));
}

function ensureTab(nav: HTMLElement) {
  if (nav.querySelector(`#${TAB_ID}`)) return;
  const files = nav.querySelector<HTMLAnchorElement>("a#prs-files-anchor-tab") ?? nav.querySelector("a");
  const tab = document.createElement("a");
  tab.id = TAB_ID;
  tab.href = "#structure";
  tab.textContent = "Structure";
  if (files) {
    tab.className = files.className;
    files.after(tab);
  } else {
    nav.append(tab);
  }
  tab.addEventListener("click", (e) => {
    e.preventDefault();
    toggle();
  });
  if (state?.open) tab.setAttribute("aria-current", "page");
}

function ensureButton() {
  if (document.getElementById(BUTTON_ID)) return;
  const b = document.createElement("button");
  b.id = BUTTON_ID;
  b.type = "button";
  b.textContent = "Structure";
  b.style.cssText =
    "position:fixed;right:16px;bottom:16px;z-index:2147483001;padding:6px 12px;border-radius:6px;border:1px solid #d0d7de;background:#f6f8fa;color:#1f2328;cursor:pointer;font:600 13px system-ui;";
  b.addEventListener("click", toggle);
  document.body.append(b);
}

function dropAll() {
  showContent();
  state?.panel.remove();
  state = null;
  document.getElementById(TAB_ID)?.remove();
  document.getElementById(BUTTON_ID)?.remove();
}

const navGuard = new WeakSet<HTMLElement>();

function sync() {
  const ref = parsePrPath(location.pathname);
  if (!ref) {
    if (state || document.getElementById(TAB_ID) || document.getElementById(BUTTON_ID)) dropAll();
    return;
  }
  const key = prParam(ref);
  if (state && state.key !== key) dropAll();
  if (!state) state = { key, panel: makePanel(key), iframe: null, open: false };
  const nav = document.querySelector<HTMLElement>(NAV_SELECTOR);
  if (nav) {
    document.getElementById(BUTTON_ID)?.remove();
    ensureTab(nav);
    if (!navGuard.has(nav)) {
      navGuard.add(nav);
      // Clicking another PR tab restores the PR's own content.
      nav.addEventListener(
        "click",
        (e) => {
          const a = (e.target as Element).closest("a");
          if (a && a.id !== TAB_ID && state?.open) setOpen(false);
        },
        true,
      );
    }
  } else {
    document.getElementById(TAB_ID)?.remove();
    ensureButton();
  }
  if (state.open) applyOpen(state);
}

let scheduled = false;
function schedule() {
  if (scheduled) return;
  scheduled = true;
  requestAnimationFrame(() => {
    scheduled = false;
    sync();
  });
}

for (const ev of ["turbo:render", "turbo:load"]) document.addEventListener(ev, schedule);
window.addEventListener("popstate", schedule);
new MutationObserver(schedule).observe(document.documentElement, { childList: true, subtree: true });
sync();
