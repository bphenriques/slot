# v1.7.0-bphenriques.1

Based on [pvaibhav's slot v1.7.0-pvaibhav.1](https://github.com/pvaibhav/slot/releases/tag/v1.7.0-pvaibhav.1), itself built on [upstream slot 1.7.0](https://github.com/BrandonKowalski/slot/releases/tag/v1.7.0), with a music and audiobook player added, under Music in the quick menu.

- **Library:** put songs in `Music/` on the card, any layout; tags win, folders fill gaps. Library has Artists, Albums and Songs, and Genres and Years once the tags name them; compilations are kept as one album. Playlists (`.m3u` in `Music/Playlists/`), Favorites, Recently Played and Recently Added. Tags are read once and cached, so the library opens at once afterwards.
- **Playing:** gapless; Crossfade only between albums, so an album played in order stays gapless; Volume Leveling from ReplayGain; an equalizer with presets from `Config/music-eq.txt`; Mono. Synced lyrics from `.lrc` files or tags, played from any line. Play Next and Add to Queue as Spotify has them; the queue, its place and its shuffle come back after a restart.
- **Audiobooks:** from `Audiobooks/`, a folder or an `.m4b` a book. Each keeps its place, speed (0.5× to 2×, the voice at its pitch) and bookmarks; chapters from `.m4b` files, MP3s' chapters or a folder's files; it picks up a little back after a pause, the longer the further; Skip Silence and Volume Boost; the sleep timer stops at the end of a chapter, going back two minutes; In Progress and Finished lists.
- **Keys:** a tap does one thing and a hold another, as the hints say ("Play (Next)"), a ring filling while one is held. Settings › Help, shown on the first start, lists the keys the hints leave out. With Screen Off set to Keep Playing, music goes on with the lid closed and the shoulders still skip, seek and pause; two shoulders held lock the keys for a pocket.
- **Card:** formats are MP3, AAC and Apple Lossless (`.m4a`, `.m4b`), FLAC, Ogg Vorbis and WAV. A card with nothing on it says where songs and books go; a write the card refuses says "Can't save".

This is a preview for feedback. Music is decoded with Symphonia (MPL-2.0), resampled with Rubato, tagged with Lofty, and played at another speed with SoundTouch (LGPL-2.1, built from its sources).

Unzip, then copy the contents of `slot-v1.7.0-bphenriques.1/` onto SD2. Your saves and settings are preserved. A card from before keeps an unused `System/muse/` and `System/muse.sh`, safe to delete.

Emulation by mGBA (MPL-2.0) and gpSP (GPL-2.0). Licences, patches, and gpSP source are in `System/licenses/`: SoundTouch's in `soundtouch-LGPL-2.1.txt`, and every crate the music player links, with where its source is, in `slot-music-third-party.txt`.
