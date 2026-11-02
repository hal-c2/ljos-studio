# Varða: a product site

A marketing site for a made-up app, Varða (offline trail maps), made to
dogfood Studio (docs/DOGFOOD.md). It shows:

- a page of sections in a `Flickable`: the nav bar (`NavBar.qml`), the
  hero with its call to action (`Hero.qml`), a features grid
  (`Features.qml`), a testimonial (`Testimonial.qml`), pricing
  (`Pricing.qml`), a closing call to action (`CallToAction.qml`) and the
  footer (`Footer.qml`);
- responsive layout through breakpoint states on the root: `phone`
  (narrower than 600), `tablet` (narrower than 1024) and `desktop`, the
  bounds Studio's preview targets use. A state sets only `columns` and
  `compact`, and each section reads those two, so a state is the one place
  that says what a size gets: one, two or three columns for features and
  pricing, and the nav links folded away on a phone;
- QtQuick and `Ljos.Design` only (tokens for colour, spacing and type), so
  the web build needs no Qt QML for the controls. `theme.json` sets the seed
  hue.

Open it in Studio with `studio samples/site` (or `mise run studio
samples/site`), open `Main.qml` and choose Desktop, Tablet or Phone: the
canvas shows the state that size gets. Run it on its own with
`ljos run samples/site/Main.qml`. For the web, export it with the tool
`export` (`"entry": "Main.qml", "target": "web"`) and build the bundle with
`mise run export:web:build samples/site/build/web`.

An agent built this site over MCP in a new, empty project on 2026-10-31: 40
calls, none refused, ending in the `export` above. `export:web:build` made a
site of 456 784 bytes, and headless Chromium showed it with no errors at 390 x
844 (one column, the nav links folded away), 800 x 1100 (two columns) and 1280 x
800 (three columns): `docs/dogfood/site-web-*.png`. `mise run export:web:check`,
which checks its own project and not this one, passed the same day.
