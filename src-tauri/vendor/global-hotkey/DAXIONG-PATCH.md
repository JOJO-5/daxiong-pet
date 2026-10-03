# Local patch to global-hotkey 0.8.0

Source: the official crates.io package, https://github.com/tauri-apps/global-hotkey.
Licenses: MIT or Apache-2.0, preserved alongside the source.

Only `src/platform_impl/x11/mod.rs::unregister_hotkey` is changed. Its
XUngrabKey cookies previously ignored errors without flushing or waiting for
X11. A separate process still received BadAccess after the app reported its
shortcut disabled. Each ungrab now calls `check()` before returning success;
errors propagate. Other platforms are unchanged.

`scripts/e2e-menu.py` verifies a second real X11 process can grab Ctrl+Alt+P
after disabling it, the app rejects the conflicting registration visibly,
and registration and rollback work after the blocker exits.
