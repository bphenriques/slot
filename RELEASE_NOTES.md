# v1.12.0-pvaibhav.2

- Adds **Authentic** LCD shaders for GB, GBC, and GBA. They draw each handheld's reflective colours, pixel grid, ghosting, and glass. Game Boy offers Game Boy, Pocket, and Light models.
- Includes the latest local shader and bezel refinements, with blue-green Game Boy middle shades, darker GBC colours, dot shadows, and a soft shadow from the bezel onto the screen.
- Selects **schedutil** for GB, GBC, gpSP, and paused play. Running GBA games on mGBA use **performance** to keep frames on time.
- Builds on upstream **v1.12.0**, keeping game guides, Reset Game with Undo Reset, Boktai sunlight, Shimmerless, and this fork's existing additions.

Pick **Authentic** under **Settings > Screen > your system > Shader**. On Game Boy, the **Model** row selects the screen. Authentic supplies its own colours and ghosting, so their separate controls are hidden while it is selected. Your existing choices are kept.

## Why schedutil now works

Authentic moves ghosting and colour effects from mGBA's CPU filters to the GPU. The tested GB and GBC games then have enough CPU time to keep frames on schedule with dynamic CPU scaling. GBA on mGBA still misses frame handoffs under schedutil, so slot selects performance for that combination and returns to schedutil when play pauses or ends.

## Authentic screenshots

Rendered from captured game frames at 720×480
with this release's shaders and bezels. The GBC look keeps the original screen's darker,
reflective colours.

<p align="center">
<img src="https://raw.githubusercontent.com/pvaibhav/slot/v1.12.0-pvaibhav.2/.github/resources/screenshots/authentic-gb.png" width="48%" alt="Metroid II with the Authentic Game Boy model, green panel, blue-green dots, and bezel shadows">
<img src="https://raw.githubusercontent.com/pvaibhav/slot/v1.12.0-pvaibhav.2/.github/resources/screenshots/authentic-gbc.png" width="48%" alt="Super Mario Bros. Deluxe with the Authentic Game Boy Color shader, reflective colours, and pixel grid">
</p>

<p align="center">Game Boy · Game Boy Color</p>

<p align="center">
<img src="https://raw.githubusercontent.com/pvaibhav/slot/v1.12.0-pvaibhav.2/.github/resources/screenshots/authentic-pocket.png" width="48%" alt="Metroid II with the Authentic Game Boy Pocket model and its grey-green reflective panel">
<img src="https://raw.githubusercontent.com/pvaibhav/slot/v1.12.0-pvaibhav.2/.github/resources/screenshots/authentic-light.png" width="48%" alt="Metroid II with the Authentic Game Boy Light model and its turquoise panel">
</p>

<p align="center">Game Boy Pocket · Game Boy Light</p>

## Install or update

Unzip `slot-v1.12.0-pvaibhav.2-h700.zip`, then copy the contents of `slot-v1.12.0-pvaibhav.2/` onto SD2. Your games, saves, settings, and credentials are preserved.

`Cheats.pack` and `Artwork.pack` are unchanged. Keep your existing copies, or put them in `Cheats/` and `Labels/` respectively.

Cheats are from the [libretro database](https://github.com/libretro/libretro-database) and its contributors, under [CC BY-SA 4.0](https://creativecommons.org/licenses/by-sa/4.0/). Cart artwork is from the [LaunchBox Games Database](https://gamesdb.launchbox-app.com) and belongs to the respective publishers; the artwork pack is for personal use.

Emulation by mGBA (MPL-2.0) and gpSP (GPL-2.0). Licences, patches, and gpSP source are in `System/licenses/`.
