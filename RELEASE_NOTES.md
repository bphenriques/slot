# v1.9.0-pvaibhav.1

- Updated to [slot v1.9.0](https://github.com/BrandonKowalski/slot/releases/tag/v1.9.0), with safer saves and better recovery from crashes, stalled cores, and damaged cards.
- Saves are synced to the SD card right away. Game saves update every minute, and automatic resume states every five minutes.
- Adds screenshots with a gallery, zoom, and multi-select, plus **Undo Save** and **Undo Load** in the in-game menu.
- Adds custom GBA bezels on 640×480 devices and fixes label caching that could stall at startup.
- Keeps this fork's automatic 120 Hz, panel calibration, battery improvements, home Wi-Fi and linking, automatic labels, and offline RetroAchievements.

Unzip `slot-v1.9.0-pvaibhav.1-h700.zip`, then copy the contents of `slot-v1.9.0-pvaibhav.1/` onto SD2. Your saves and settings are preserved.

For labels without Wi-Fi, copy `Artwork.pack` below into `Labels/` on your SD card. The pack is unchanged from the previous release. Existing labels are kept.

Cart images in `Artwork.pack` come from the [LaunchBox Games Database](https://gamesdb.launchbox-app.com) and its contributors. Label artwork belongs to each game's publisher. The pack is free for personal use; please don't sell it.

Emulation by mGBA (MPL-2.0) and gpSP (GPL-2.0). Licences, patches, and gpSP source are in `System/licenses/`.
