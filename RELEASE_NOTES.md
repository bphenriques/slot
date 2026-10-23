# v1.8.1-pvaibhav.1

- Updated to [slot v1.8.1](https://github.com/BrandonKowalski/slot/releases/tag/v1.8.1), fixing freezes in MBC2 Game Boy games, shelf navigation, and the BaseOS startup sound.
- Adds separate **Load State on Insert** and **Save State on Eject** settings, plus **Off** for fast-forward. Save states and rewind use less space.
- Keeps this fork's automatic 120 Hz, panel calibration, battery improvements, home Wi-Fi and linking, automatic labels, and offline RetroAchievements.

If you previously disabled Auto Save on Eject for fresh starts, also turn off **Load State on Insert**.

Unzip `slot-v1.8.1-pvaibhav.1-h700.zip`, then copy the contents of `slot-v1.8.1-pvaibhav.1/` onto SD2. Your saves and settings are preserved.

For labels without Wi-Fi, copy `Artwork.pack` below into `Labels/` on your SD card. The pack is unchanged from the previous release. Existing labels are kept.

Cart images in `Artwork.pack` come from the [LaunchBox Games Database](https://gamesdb.launchbox-app.com) and its contributors. Label artwork belongs to each game's publisher. The pack is free for personal use; please don't sell it.

Emulation by mGBA (MPL-2.0) and gpSP (GPL-2.0). Licences, patches, and gpSP source are in `System/licenses/`.
