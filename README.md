<div align="center">

<img src=".github/resources/banner.png" width="100%" alt="slot. A bespoke, Game Boy-centric frontend for the Anbernic RG SP.">

<a href="LICENSE"><img src="https://img.shields.io/github/license/BrandonKowalski/slot?style=for-the-badge&labelColor=242429&color=4d4d57" height="36" alt="License"></a>
<a href="https://github.com/BrandonKowalski/slot/stargazers"><img src="https://img.shields.io/github/stars/BrandonKowalski/slot?style=for-the-badge&labelColor=242429&color=4d4d57" height="36" alt="Stars"></a>
<a href="https://github.com/BrandonKowalski/slot/releases"><img src="https://img.shields.io/github/downloads/BrandonKowalski/slot/total?style=for-the-badge&labelColor=242429&color=4d4d57" height="36" alt="Downloads"></a>
<a href="https://slot-cfw.fyi"><img src="https://img.shields.io/badge/Guide-slot--cfw.fyi-4d4d57?style=for-the-badge&labelColor=242429" height="36" alt="Guide"></a>
<a href="CHANGELOG.md"><img src="https://img.shields.io/badge/Changelog-4d4d57?style=for-the-badge&labelColor=242429" height="36" alt="Changelog"></a>

</div>

---

# What is slot?

A fork of [slot](https://github.com/BrandonKowalski/slot) by Brandon Kowalski. slot is a
bespoke, Game Boy-centric frontend for the Anbernic RG SP. Brandon wrote slot and its guide.
This fork follows upstream and adds the features listed below.

Has support for GBA, GBC, and GB titles only.

> [!IMPORTANT]
> ROMs must be unzipped. slot does not read `.zip` or `.7z` files.

The user guide is upstream's, at [slot-cfw.fyi](https://slot-cfw.fyi). Upstream's release
notes are in the [changelog](CHANGELOG.md).

## What this fork adds

- **Lower input lag with automatic 120 Hz** on supported RG SP, RG34XX SP, and RG34XX
  panels. Other panel revisions keep their existing refresh rate.
- **Better battery life and less heat.** slot switches the CPU to the `schedutil` governor, so
  the clock follows the game's load. Measured on an RG SP: Game Boy and Game Boy Color games run
  at about 600 MHz, and GBA at 550–1,000 MHz, out of a 1,512 MHz maximum.
- **LCD ghosting / interframe blending** is enabled by default. It's subtle.
- **Colour calibration** for the RG SP and RG34XXSP panels, based on an accurate 3x1D LUT
  measured by me. Both white point and gamma curve are corrected. Currently no other firmware
  has this!
- Holding POWER **shuts down** directly, without showing a menu. I did not find value in a
  restart option.
- **Home Wi-Fi.** Slot can now connect to your home Wi-Fi. Just set up your Wi-Fi credentials
  in `Config/wifi.toml` and then turn HOME WI-FI on under General. Everything happens
  automatically from then onwards. Most of the features below depend on this.
- **Hassle-free cart label scraping.** Just throw your ROMs on the SD card, and slot downloads
  missing labels in the background. It will not replace a label you added yourself, though.
  No config is needed for this feature. No Wi-Fi? Put `Artwork.pack` from the release page in
  your card's `Labels` folder and slot labels your carts from that instead.
- **RetroAchievements.** You can earn softcore GB, GBC, and GBA achievements, online or offline.
  Put your account in `Config/retroachievements.toml` for this to work. All your games' achievements
  will be cached if Wi-Fi is on, so once the sync is done, you can go out and play without
  losing your achievements. They'll sync automatically when you're back home. When you're
  playing at home with Wi-Fi on, your achievements will sync in real time with rich presence.
- **Linking over home Wi-Fi.** When at home and connected to Wi-Fi, two handhelds can link
  using that instead of having to create ad hoc networks. Outside of home, you can continue
  using slot's normal method. This works seamlessly, no config needed.
- **ROMs in subfolders.** Slot also finds games one folder down, like `Games/GBA/Homebrew/`, so
  a library sorted into folders still shows on the shelf.
- **Music and audiobook player.** QUICK MENU > MUSIC plays what is in `Music/` and
  `Audiobooks/`, offline. Browse by artist, album, song, genre, year or folder, with album art
  and `.m3u` playlists; play gapless, with Volume Leveling, an equalizer and synced lyrics. Books
  keep their place, speed, chapters and bookmarks. Close the lid and it plays on, the shoulders
  still skipping and pausing. MP3, AAC, ALAC, FLAC, Ogg Vorbis, Opus and WAV.

## How to configure

The release includes an `.example` copy of each config file in `Config/`. Remove `.example`
from the name, and fill in your details. You only need to configure your Wi-Fi and
RetroAchievements credentials. Everything else is automatic.

## Versions

This fork adds a music player to [pvaibhav's fork](https://github.com/pvaibhav/slot). A release
uses upstream's version with a `-bphenriques` suffix: `v1.7.0-bphenriques.1` is the first
built on upstream 1.7.0, and `v1.7.0-bphenriques.2` would be the second.

The current release is [v1.8.0-bphenriques.1](https://github.com/bphenriques/slot/releases/tag/v1.8.0-bphenriques.1).
Its [release notes](RELEASE_NOTES.md) describe the changes since the previous fork release.

---

# What does it look like?

<p align="center">
<img src=".github/resources/screenshots/carousel.gif" width="48%" alt="Scrolling the cart carousel across the Game Boy Advance, Game Boy and Game Boy Color shelves">
<img src=".github/resources/screenshots/core-picker.gif" width="48%" alt="Flipping a cart to its circuit board to choose mGBA or gpSP">
</p>

<p align="center">
<img src=".github/resources/screenshots/link.gif" width="96.5%" alt="Two SPs racing in Mario Kart over the emulated link">
</p>

The music player:

<p align="center">
<img src=".github/resources/screenshots/music-01-music.png" width="48%" alt="The Music menu: Shuffle Library, Queue, Library, Playlists, Audiobooks and Settings">
<img src=".github/resources/screenshots/music-05-albums.png" width="48%" alt="Albums, each with its cover, under Play All">
</p>

<p align="center">
<img src=".github/resources/screenshots/music-12-now-playing.png" width="48%" alt="Now Playing: the album cover, the song, its artist and album, and Up Next">
<img src=".github/resources/screenshots/music-19-lyrics.png" width="48%" alt="Synced lyrics, the line being sung lit">
</p>

<p align="center">
<img src=".github/resources/screenshots/music-15-queue.png" width="48%" alt="The queue, the song playing marked on its cover">
<img src=".github/resources/screenshots/music-26-book.png" width="48%" alt="An audiobook playing: the chapter, the next one, and the time the book has left">
</p>

---

# Supported Devices

| Device             | Supported                                    | Since Version |
|--------------------|----------------------------------------------|---------------|
| Anbernic RG SP     | Yes                                          | Always        |
| Anbernic RG34XX    | Yes                                          | 1.4.0         |
| Anbernic RG34XXSP  | Yes (Needs Left Stick as D-Pad Verification) | 1.4.0         |
| Anbernic RG35XXSP  | Yes                                          | 1.5.0         |
| Anbernic 35XX Plus | Yes                                          | 1.5.0         |
| Anbernic RG28XX    | Yes                                          | 1.6.0         |
| Anbernic RG35XX-H  | Yes                                          | 1.7.0         |

If you have one of the untested devices, or have tried slot on a device not listed,
please [open an issue on GitHub](https://github.com/BrandonKowalski/slot/issues) and let me know how it went.

---

# AI Disclosure

From upstream:

The Rust frontend, and the SVGs on [slot-cfw.fyi](https://slot-cfw.fyi), were put together by Claude.
I reviewed everything that was produced. All documentation is 100% free-range, meatbag prose.

My goal with this project was to recreate the feeling of playing my GBA SP in the back seat as a kid,
but with a few modern creature comforts.

Nothing about this project is exciting or groundbreaking from a technological perspective, and as a
result, I delegated the glue code. mGBA and gpSP have been around forever and are the true stars of
this show. The incredible skeuomorphic art is the work of [Markus](https://github.com/MarkusMaribu), an
artist who saw the bare-bones version and offered to dress it up.

I understand if my use of AI is a dealbreaker for you. Just know it's the least interesting part of
what's here.

This fork's additions were written with AI assistance too.

Figured I should share the end result of all the wasted water. ✌🏻

---

# Spread Joy!

I've spent a lot of time building slot.

If you enjoy using it and feel inclined to pay it forward, go do something nice for someone! ❤️
