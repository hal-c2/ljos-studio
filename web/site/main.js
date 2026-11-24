// The page a site is: the entry's root, as wide as the window, and as tall
// as what it shows, so the page scrolls as a document does (a reader's
// wheel, keys, scroll bar and "find" are the browser's) where a window
// would scroll inside itself.
import "ljos-web/runtime.css";
import { mount } from "ljos-web/object";
import Entry from "ljos-site:entry";

const app = document.getElementById("app");
const { object: root } = mount(Entry, app);

const isFlickable = (o) => o && "contentHeight" in o && "interactive" in o;
const childrenOf = (o) => {
  try {
    return [...(o.children ?? [])];
  } catch {
    return [];
  }
};
// What scrolls the page in a window: the root itself, or a Flickable that
// fills it (`anchors.fill: parent`), as a page in Studio is made.
const scroller = () =>
  isFlickable(root) ? root : childrenOf(root).find((c) => isFlickable(c) && c.width === root.width && c.height === root.height);

// The element a Flickable scrolls in: the outermost below its own.
const viewportOf = (flick) => {
  const node = flick.$node;
  if (!(node instanceof Element)) return app.querySelector("qq-flick");
  return node.localName === "qq-flick" ? node : node.querySelector("qq-flick");
};

// How tall the root's content is: its scroller's content, else what the
// root says of itself.
const contentHeight = (flick) => {
  if (flick) return flick.contentHeight + (flick.topMargin ?? 0) + (flick.bottomMargin ?? 0);
  const rect = root.childrenRect;
  return Math.max(root.implicitHeight || 0, rect ? rect.y + rect.height : 0);
};

// A root whose content grows with its height (a picture bound to
// `root.height`) would grow with each fit: after a few fits that grew by
// as much as the root did, the page is the window's height and scrolls
// inside, as it would in a window.
let windowed = false;
let fits = 0;
let fitting = false;
const fit = () => {
  if (fitting) return;
  fitting = true;
  try {
    root.width = window.innerWidth;
    const flick = scroller();
    let tall = windowed ? window.innerHeight : Math.max(window.innerHeight, Math.ceil(contentHeight(flick)));
    if (!windowed && tall > window.innerHeight && tall > root.height && root.height > window.innerHeight && ++fits > 3) {
      // This pass goes on as the window's height: a fit started from here
      // would stop at `fitting` and leave the root as it was.
      windowed = true;
      tall = window.innerHeight;
    }
    root.height = tall;
    app.style.height = `${tall}px`;
    if (flick) {
      if (!windowed) flick.contentY = 0;
      flick.interactive = windowed;
      // ljos-web keeps a reader's wheel in a Flickable's element even when
      // it does not scroll (`overscroll-behavior`), so the page's element
      // is marked for index.html to let the wheel on to the document.
      const viewport = viewportOf(flick);
      if (viewport) viewport.toggleAttribute("data-ljos-page", !windowed);
    }
  } finally {
    fitting = false;
  }
};

const hear = (o, name) => o?.[`${name}Changed`]?.connect?.(() => queueMicrotask(fit));
window.addEventListener("resize", () => {
  fits = 0;
  windowed = false;
  fit();
});
fit();
const flick = scroller();
if (flick) hear(flick, "contentHeight");
else {
  hear(root, "implicitHeight");
  hear(root, "childrenRect");
}

// ljos-web says `Accessible.Link` on an item's element as `role="link"` and
// makes no `a` of it, so the page makes one of an item that has that role
// and an `href`: what a browser follows and opens in a tab, and a crawler
// reads. An address of another scheme (`javascript:`, `file:`) is no link
// of a site.
const FOLLOWED = new Set(["http:", "https:", "mailto:", "tel:"]);
const addressOf = (item) => {
  const given = String(item.href ?? "");
  try {
    return given && FOLLOWED.has(new URL(given, location.href).protocol) ? given : null;
  } catch {
    return null;
  }
};

const anchors = new WeakMap();
const heard = new WeakSet();
const anchor = (item) => {
  const node = item.$node;
  const address = addressOf(item);
  let a = anchors.get(node);
  if (!address) return a?.removeAttribute("href");
  if (!a) {
    a = document.createElement("a");
    a.dataset.ljosLink = "";
    // A Text's letters go inside, so the link reads as them. Anything else
    // is covered by it.
    const letters = node.querySelector(":scope > .qq-text");
    node.prepend(a);
    if (letters) a.append(letters);
    anchors.set(node, a);
  }
  a.setAttribute("href", address);
  if (node.getAttribute("role") !== "link") return;
  // One link to a reader, not one inside another.
  const name = node.getAttribute("aria-label");
  if (name) a.setAttribute("aria-label", name);
  node.removeAttribute("role");
  node.removeAttribute("aria-label");
};

const linkUp = () => {
  const seen = new Set();
  const walk = (item) => {
    if (!item || seen.has(item)) return;
    seen.add(item);
    const node = item.$node;
    if (node instanceof Element && node.getAttribute("role") === "link" && "href" in item) {
      anchor(item);
      if (!heard.has(item)) {
        heard.add(item);
        item.hrefChanged?.connect?.(() => queueMicrotask(() => anchor(item)));
      }
    }
    for (const child of childrenOf(item)) walk(child);
  };
  walk(root);
};

// The tree is walked again only when an element becomes a link or one
// arrives, so a page at rest does no work for it.
const bringsLink = (record) =>
  record.type === "attributes"
    ? record.target.getAttribute("role") === "link"
    : [...record.addedNodes].some((n) => n instanceof Element && (n.matches('[role="link"]') || n.querySelector('[role="link"]')));
linkUp();
new MutationObserver((records) => records.some(bringsLink) && linkUp()).observe(app, {
  subtree: true,
  childList: true,
  attributeFilter: ["role"],
});
