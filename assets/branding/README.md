# QuotaTide visual assets

The source artwork in this directory is original QuotaTide project artwork.
It is distributed under the repository's MIT license.

## Chosen direction

Direction A, **Tide Dial**, was selected on 2026-07-28. The mark combines:

- a circular quota gauge;
- a rising tide representing consumption and reset;
- seven ticks in the large application icon for the active seven-day window.

The small application icon intentionally removes ticks and thin highlights below
48 px. The tray glyph removes all decorative detail and retains only the gauge
ring and tide silhouette.

## Sources

| File | Purpose |
|---|---|
| `app-icon.svg` | Canonical 1024 px application artwork |
| `app-icon-small.svg` | Optical variant for 16–64 px application layers |
| `tray-template.svg` | Black-alpha macOS template/high-contrast glyph |
| `tray-template-inverse.svg` | White-alpha Windows dark/high-contrast glyph |
| `tray-color.svg` | Default Windows tray icon |

Do not embed the QuotaTide name, the Chinese subtitle, or any OpenAI/Codex mark
inside the artwork.

## Generated outputs

Run:

```sh
npm run icons
```

The generator writes Tauri application icons to `src-tauri/icons` and runtime
tray assets to `src-tauri/icons/tray`. Generated raster files must not be edited
by hand.
