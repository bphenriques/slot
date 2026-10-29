# v1.10.0-pvaibhav.1

- Updates to upstream **v1.10.0**, with picture scaling and stretching options, matching Game Boy bezels, and a **GBA Video** shelf for carts in `Games/GBAV`.
- Date & Time now uses cities, handles daylight saving automatically, and offers a 12-hour clock. Includes upstream's brightness, menu and shutdown fixes.
- Upstream's **Frame Blending** replaces the old LCD Ghosting toggle. Existing On settings become **Mix**, preserving their appearance; Off stays Off. mGBA also offers **LCD Ghosting**.
- Keeps this fork's automatic 120 Hz, display calibration, home Wi-Fi, label downloads, RetroAchievements, home-network linking and cheats. The CPU governor now uses upstream's implementation.

Unzip `slot-v1.10.0-pvaibhav.1-h700.zip`, then copy the contents of `slot-v1.10.0-pvaibhav.1/` onto SD2. Your games, saves, settings and credentials are preserved.

`Cheats.pack` and `Artwork.pack` are unchanged. Keep your existing copies, or put them in `Cheats/` and `Labels/` respectively.

Cheats are from the [libretro database](https://github.com/libretro/libretro-database) and its contributors, under [CC BY-SA 4.0](https://creativecommons.org/licenses/by-sa/4.0/). Cart artwork is from the [LaunchBox Games Database](https://gamesdb.launchbox-app.com) and belongs to the respective publishers; the artwork pack is for personal use.

Emulation by mGBA (MPL-2.0) and gpSP (GPL-2.0). Licences, patches, and gpSP source are in `System/licenses/`.
