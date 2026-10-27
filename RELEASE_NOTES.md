# v1.9.0-pvaibhav.3

- Adds **cheats**. Copy `Cheats.pack` below into `Cheats/` on your SD card. While a game is running, press MENU and choose **Cheats** to turn its cheats on or off. Changes take effect immediately, and slot remembers which cheats are on for each game.
- The pack has cheats for 2,503 GB, GBC, and GBA games, matched to your ROMs by checksum, so file names don't matter.
- To use your own cheats for a game, add a RetroArch `.cht` file named after the ROM to `Cheats/GBA`, `Cheats/GB`, or `Cheats/GBC`. It replaces that game's cheats from the pack.
- Cheats work with the mGBA core. The **Cheats** row appears only for games that have cheats. A cheat that changes items or money can be written into your save, even after you turn it off.

![The Cheats page over Super Mario Bros. Deluxe, with Always Have Fire turned on.](https://raw.githubusercontent.com/pvaibhav/slot/v1.9.0-pvaibhav.3/.github/resources/screenshots/cheats.png)

Unzip `slot-v1.9.0-pvaibhav.3-h700.zip`, then copy the contents of `slot-v1.9.0-pvaibhav.3/` onto SD2. Your saves and settings are preserved.

For cheats, copy `Cheats.pack` below into `Cheats/` on your SD card.

For labels without Wi-Fi, copy `Artwork.pack` below into `Labels/` on your SD card. The pack is unchanged from the previous release. Existing labels are kept.

Cheat codes in `Cheats.pack` come from the [libretro database](https://github.com/libretro/libretro-database) and its contributors, licensed under [CC BY-SA 4.0](https://creativecommons.org/licenses/by-sa/4.0/). slot matched them to No-Intro dumps and merged each game's files; `Cheats.pack` is shared under the same licence.

Cart images in `Artwork.pack` come from the [LaunchBox Games Database](https://gamesdb.launchbox-app.com) and its contributors. Label artwork belongs to each game's publisher. The pack is free for personal use; please don't sell it.

Emulation by mGBA (MPL-2.0) and gpSP (GPL-2.0). Licences, patches, and gpSP source are in `System/licenses/`.
