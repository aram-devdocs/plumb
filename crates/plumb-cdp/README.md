# plumb-cdp

Chromium DevTools Protocol driver for [Plumb](https://plumb.aramhammoudeh.com) —
renders pages to `PlumbSnapshot`.

This crate drives a headless Chromium instance via CDP to capture the
computed DOM at one or more viewports. It is the only crate in the Plumb
workspace permitted to use `unsafe` code.

## Key types

- `BrowserDriver` — trait abstracting snapshot capture.
- `ChromiumDriver` — real Chromium driver (requires a local Chromium binary).
- `FakeDriver` — deterministic in-memory driver for `plumb-fake://` URLs.
- `PersistentBrowser` — reusable browser session for MCP server use.

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE)
or [MIT License](LICENSE-MIT) at your option.
