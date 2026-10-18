# Music controls

The hints at the bottom of each screen show what the buttons do there. A button that does
something else when held shows it in brackets, "Play (Next)": while it is held, a ring fills in
the top right corner; let go before the ring closes and nothing happens. This page also lists what the hints leave out.

## Everywhere in Music

| Button | Tapped | Held |
|---|---|---|
| B | Back a screen (lists and Now Playing leave it out of their hints). | Back to the Music menu. |
| START | The menu for what is shown, where it has one: Now Playing, a song, a book. | Pause or play; away from Now Playing, the corner says which. |
| SELECT | Now Playing, and from there the Queue, opened at the song playing with those played above it, faint, as Apple Music has it (a book's chapters). | |
| SELECT + L1 / R1 | Previous / next song. | |
| MENU | Back to slot. | |

## Music menu

| Button | Tapped | Held |
|---|---|---|
| A | Open. | On Artists, Albums or Songs: shuffle them all. On Audiobooks: play on the book listened to last. |

## Lists (songs, albums, artists, playlists)

| Button | Tapped | Held |
|---|---|---|
| A | Open, or play from this song. | Play the row's songs next. |
| X | Add the row's songs to the queue: after the song playing and those added before, ahead of the rest, as Spotify and Apple Music do. | |
| Y | Add the song to Favorites, or take it out (a heart marks it). | |
| START | On a song: Go to Album, Go to Artist, Song Info. | |
| Left / Right | A page back / on. | Keep turning. |
| L1 / R1 | The previous / next letter, shown large for a moment. | Keep jumping. |

## Playlists

Playlists are `.m3u` files in `Music/Playlists`, read with the library; Favorites is one,
written as Y adds and takes out songs. The others are only played here, not edited: changing
a list a song at a time with a pad is slow, and a song taken out by mistake is easily lost
for good. Edit them on a computer. The queue can be kept as a new
playlist, from START in the Queue, to play and not to edit.

First in Playlists come Favorites, Recently Played (the last 100 songs, newest first, each
once; a song counts once it has played 30 seconds, so a skip does not, and books never do;
kept in `Config/music-recent.m3u8`) and Recently Added (albums, newest first).

## Now Playing

| Button | Tapped | Held |
|---|---|---|
| A | Pause or play. | |
| X | The lyrics, full screen; the hint shows only when the song has some. | |
| Y | Add the song to Favorites, or take it out. | |
| Up | Repeat: Off, All, One, round again (a symbol shows which). | |
| Down | Shuffle the queue, or put it back in order. | |
| Left / Right | Previous / next song. | Seek back / on. |
| L1 / R1 | Previous / next song. Previous starts the song over after its first 3 seconds. Paused, the song skipped to waits, paused. | Seek back / on. |
| L2 / R2 | Seek back / on 10 seconds, as they are let go. | Keep seeking. |
| START | Favorites, Sleep Timer (Off, End of Song, 5 to 45 min, 1 h, 2 h; Now Playing's corner counts it down; play pressed while it fades the music out brings it back and starts the timer again), Repeat (A goes round Off, All, One), Shuffle (On or Off, as Down does), Go to the album or artist, Song Info. | |

## Lyrics

X on Now Playing shows the song's lyrics: an `.lrc` file beside it with the same name, or the
lyrics in its tags, as beets writes them. Timed lyrics (LRC) follow the song, the line sung lit
in the middle; plain lyrics are to read. X or B closes them. Scrolled to a line of timed
lyrics, A plays from it.

| Button | Tapped | Held |
|---|---|---|
| Up / Down | Scroll; timed lyrics follow the song again a few seconds after. | Keep scrolling. |
| A, L1 / R1, Left / Right, L2 / R2 | As on Now Playing. | As on Now Playing. |

## The Queue

| Button | Tapped | Held |
|---|---|---|
| A | Play from this song, or start the playing one over. | Play the song next. |
| X | Take the song out. | Go on taking out the songs after it, quicker and quicker and then several at a time, up to the one playing or the last. |
| Y | Add the song to Favorites, or take it out, as in every list. | Pick the song up to move it, as a long press drags a row on a phone: Up and Down carry it, Y puts it down, R1 puts it next, B puts it back. |
| Left / Right, L1 / R1 | A page back / on (the queue has no letters to jump by). | Keep turning. |
| START | Save as Playlist (a new "Saved Queue" in `Music/Playlists`, the whole queue in order), or Clear Queue (all but the song playing). | |

## Audiobooks

Put books in `Audiobooks/` on the card, next to `Music/`: a folder per book (its files play in
order, each a chapter), or a single `.m4b` (its own chapters). A folder named
`Author - Title` names a book whose tags do not. Books stay out of Artists, Albums and Songs.

Audiobooks lists the book you are listening to first, then those not begun, then those
finished, with how far into each you are. Each book keeps its place, speed and bookmarks in
`Config/audiobooks.txt`. An `.m4b`'s chapters are read from it, as are an MP3's ID3 chapters; a book of
other files has a chapter a file. Now Playing shows the chapter on the bar, and under the picture the speed, how far
through the whole book you are and the time it has left. Playing on after a pause steps
back a little, more the longer it was paused: 5 seconds after a few minutes, up to 30 after an
hour or more. Stopped by the sleep timer, a book goes back 2 minutes more, for what you missed
dozing off.

Your music's queue waits while a book plays: songs added meanwhile join it, and it comes back
when the book ends, or when you play music.

| Button | Tapped | Held |
|---|---|---|
| A (list) | Play on where it was left. | |
| Y (list) | The book's bookmarks. | |
| START (list) | Start Over, or Mark as Finished / Unfinished. | |
| A | Pause or play. | |
| Left / Right | Jump back / on, as far as Jump Length in Music Settings says (10, 15 or 30 seconds). | Seek back / on. |
| L1 / R1 | Previous / next chapter. Previous starts the chapter over after its first 3 seconds. | |
| L2 / R2 | Jump back / on, as Left / Right. | Keep jumping. |
| X | The next speed: 0.75×, 1×, 1.25×, 1.5×, 1.75×, 2×, round again. START's Speed row also has 0.5× (Left). | |
| Y | Bookmark the moment: a tick marks it on the bar. | The bookmarks. |
| SELECT | The chapters, to play from. | |
| START | Back to where you were before a skip, a chapter or a bookmark moved you; Chapters; Bookmarks (A plays from one, X takes it out); Speed; Sleep Timer (End of Chapter, or minutes, counted down in the corner); Mark as Finished. | |

## Pocket mode

L1 + R1 or L2 + R2 together, unless Pocket Lock in Music Settings is Off, work
with the screen on or off. Tapped, they play or pause; held until the corner ring closes, they
lock the music's other keys, so nothing changes in a pocket, or unlock them; turning the screen
on (opening the lid) unlocks them too. A lock buzzes once and an unlock
twice (Pocket Vibration) and the corner says Locked or Unlocked. On their own, the shoulders and
triggers act as they are let go, so the first of the two pressed does nothing.

## Screen off

With `screen_off = play`, the shoulders and triggers still work with the lid closed, as on
Now Playing; play and pause are the pocket combo's (tapped):

| Button | Tapped | Held |
|---|---|---|
| L1 / R1 | Previous / next song; in an audiobook, the chapter. | Seek back / on. |
| L2 / R2 | Seek back / on 10 seconds; in an audiobook, jump as far as Jump Length. | Seek back / on. |

Paused or stopped, slot powers off after its screen-off time.

## Settings

Theme follows slot's Appearance (Slot), or keeps the music screens Dark or Light. Songs always follow on with no gap, and a Sound page holds Crossfade (3 or 6 s), Volume Leveling (from the files' ReplayGain tags: an album as a whole when played in order, each song when shuffled) and Mono Audio (both sides alike, for one ear or one speaker). With audiobooks on the card, an Audiobooks page holds Jump Length, Skip Silence (pauses cut short) and Volume Boost (+3 or +6 dB). The clock and the "Up Next" line under Now Playing are set in `Config/music.txt`.

Hold A on Rebuild Library to read every song again. Update Library reads only what changed.

Last in Settings, a Debug page holds what helps say how the device does: Test Vibration (A buzzes
once, as a lock does), Vibration Length (120, 250, 400 or 600 ms a buzz, for the pocket lock
too; `buzz` in `music.txt`), Performance Log (while on, a line every 10 seconds in `Logs/music-perf.log` on the card: frames a second, how long the music's frame, its screen and its drawing take, each thread's share of the processor and the memory held, and what was shown; send it over to say how the device does) and Last Error (why the sound last failed: a file, or the device).
