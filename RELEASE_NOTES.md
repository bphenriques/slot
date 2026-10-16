# v1.7.0-bphenriques.1

Based on [pvaibhav's slot v1.7.0-pvaibhav.1](https://github.com/pvaibhav/slot/releases/tag/v1.7.0-pvaibhav.1), itself built on [upstream slot 1.7.0](https://github.com/BrandonKowalski/slot/releases/tag/v1.7.0), with a music player added.

- A music player under Music in the quick menu, played through plorpOS's Muse. Browse Artists, Albums, Songs, Playlists (`.m3u`), Favorites and Recently Added; Now Playing shows the album picture, and the queue (Up Next) can be reordered.
- Put music in `Music/` on the card. Tags are read once and cached, so the library opens at once afterwards.
- Buttons do one thing tapped and another held, as the hints say ("Play, Hold Next"); a ring in the corner fills while a button is held, and letting go before it closes cancels. [CONTROLS.md](https://github.com/bphenriques/slot/blob/pvaibhav-improvements/music/CONTROLS.md) lists every button.
- Audiobooks, from `Audiobooks/` on the card, apart from the music: each keeps its place, speed (0.75× to 2×) and bookmarks; chapters come from `.m4b` files or a folder's files; Left / Right jump 30 seconds; the sleep timer can stop at the end of a chapter; and the music's queue waits while a book plays.
- A Music Settings page, saved in `Config/music.txt`: list size, Dynamic Color, Theme (follows slot's Appearance, or dark or light), and whether music plays on with the screen off. The clock and Up Next are set in the file.

This is a preview for feedback. Music plays through Muse, plorpOS's audio daemon (MIT), with FFmpeg 6.1 (LGPL-2.1), both bundled in `System/muse/`.

Unzip, then copy the contents of `slot-v1.7.0-bphenriques.1/` onto SD2. Your saves and settings are preserved.

Emulation by mGBA (MPL-2.0) and gpSP (GPL-2.0). Licences, patches, and gpSP source are in `System/licenses/`; Muse's and FFmpeg's, with where to get FFmpeg's source, in `System/licenses/muse/`.
