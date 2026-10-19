# Music

The hints at the bottom of each screen say what the buttons do there; "Play (Next)" is a tap
and a hold, and a ring fills while a key is held. Settings › Controls lists the keys the hints
leave out, and Settings › Help the rest in a few lines. This page is what the screens cannot
say: where things go on the card, and how the player behaves.

## On the card

- `Music/`: songs, a folder per artist and album or any layout; tags win, folders fill gaps.
- `Music/Playlists/`: `.m3u` playlists, played here and edited on a computer. Favorites is one,
  written as Y adds and takes out songs; START in the Queue saves it as a new one.
- `Audiobooks/`: a folder per book (its files play in order, each a chapter), or one `.m4b` (its
  own chapters; an MP3's ID3 chapters are read too). `Author - Title` names a book with no tags.
- Lyrics: an `.lrc` beside the song with its name, or the lyrics in its tags.
- `Config/music.txt`: the settings, kept as they change. Up Next (`up_next = off`) is set only
  here; see `music.txt.example`.
- `Config/music-eq.txt`: your equalizer presets; copy `music-eq.txt.example` to start.
- `Config/audiobooks.txt`: each book's place, speed and bookmarks.
- `.scrobbler.log`: songs listened to, for a Last.fm uploader, with Settings › Integrations on.
- `Logs/slot.log`: what went wrong, and with Settings › Debug › Detailed Log, what happened.

Formats: MP3, AAC and Apple Lossless (`.m4a`, `.m4b`, `.aac`), FLAC, Ogg Vorbis and WAV.

## How it plays

- Songs follow on with no gap. What is added to the queue plays after the song playing and what
  was added before, ahead of the rest, as Spotify and Apple Music do.
- Previous starts a song, or a chapter, over after its first 3 seconds. Skipped to while paused,
  a song waits, paused.
- Names sort as players have them: a leading "The" does not count, digits and symbols come first.
- Recently Played keeps the last 100 songs, each once it has played 30 seconds; books never.
- Volume Leveling uses ReplayGain tags: the album's when played in order, each song's shuffled.
- The sleep timer fades out to a pause; play during the fade brings it back and starts again.

## Audiobooks

- A book picks up where it was left, a little back after a pause: 5 seconds after a few
  minutes, up to 30 after an hour; 2 minutes more when the sleep timer stopped it.
- A book never repeats: it plays through its parts and ends, marked finished.
- The music's queue waits while a book plays, and comes back when it ends or music plays.

## Pocket and screen off

- Two of L1, R1, L2 and R2 held together on Now Playing, or with the lid closed, lock the other
  keys, or unlock them; a buzz says which (Pocket Lock and Pocket Vibration in Settings).
- With Screen Off set to Keep Playing, music goes on with the lid closed: L1 and R1 skip (in a
  book, jump), held they seek; L2 and R2 play or pause, held in a book they bookmark. Paused,
  slot powers off after its screen-off time.
- SELECT held blanks the screen with the lid open; any button lights it and does what it does.
