# Music controls

The hints at the bottom of each screen show what the buttons do there. A button that does
something else when held shows it in brackets, "Play (Next)": while it is held, a ring fills in
the top right corner; let go before the ring closes and nothing happens. This page also lists what the hints leave out.

## Everywhere in Music

| Button | Tapped | Held |
|---|---|---|
| B | Back a screen (lists and Now Playing leave it out of their hints). | Back to the Music menu. |
| START | The menu for what is shown, where it has one: Now Playing, a song, a book. | Pause or play; away from Now Playing, the corner says which. |
| SELECT | Now Playing, and from there Up Next (a book's chapters). | |
| SELECT + L1 / R1 | Previous / next song. | |
| MENU | Back to slot. | |

## Music menu

| Button | Tapped | Held |
|---|---|---|
| A | Open. | On Artists, Albums or Songs: shuffle them all. On Audiobooks: play on the book listened to last. |
| X | | On Now Playing: stop, and empty the queue to start afresh (a book keeps its place). |

## Lists (songs, albums, artists, playlists)

| Button | Tapped | Held |
|---|---|---|
| A | Open, or play from this song. | Play the row's songs next. |
| X | Add the row's songs to the end of the queue. | |
| Y | Add the song to Favorites, or take it out (a heart marks it). | |
| START | On a song: Go to Album, Go to Artist, Song Info. | |
| Left / Right | A page back / on. | Keep turning. |
| L1 / R1 | The previous / next letter, shown large for a moment. | Keep jumping. |

## Now Playing

| Button | Tapped | Held |
|---|---|---|
| A | Pause or play. | |
| X | Shuffle the queue, or put it back in order. | |
| Y | Add the song to Favorites, or take it out. | |
| Left / Right | Previous / next song. | Seek back / on. |
| L1 / R1 | Previous / next song. Previous starts the song over after its first second. | |
| L2 / R2 | Seek back / on 10 seconds. | Keep seeking. |
| START | Favorites, Sleep Timer, Repeat (A goes round Off, All, One), Go to the album or artist, Song Info. | |

## Up Next (the queue)

| Button | Tapped | Held |
|---|---|---|
| A | Play from this song, or start the playing one over. | Play the song next. |
| X | Take the song out. | Go on taking out the songs after it, quicker and quicker, up to the one playing or the last. |
| Y | Pick the song up to move it: Up and Down carry it, Y puts it down, R1 puts it next, B puts it back. | Add the song to Favorites, or take it out. |
| L1 / R1 | A page back / on. | |

## Audiobooks

Put books in `Audiobooks/` on the card, next to `Music/`: a folder per book (its files play in
order, each a chapter), or a single `.m4b` (its own chapters). A folder named
`Author - Title` names a book whose tags do not. Books stay out of Artists, Albums and Songs.

Audiobooks lists the book you are listening to first, then those not begun, then those
finished, with how far into each you are. Each book keeps its place, speed and bookmarks in
`Config/audiobooks.txt`. An `.m4b`'s chapters are read from it; other files' are kept once they
have played. Now Playing shows the chapter on the bar, and under the picture the speed, how far
through the whole book you are and the time it has left. Playing on after a pause steps
back a little, more the longer it was paused: 5 seconds after a few minutes, up to 30 after an
hour or more.

Your music's queue waits while a book plays: songs added meanwhile join it, and it comes back
when the book ends, or when you play music.

| Button | Tapped | Held |
|---|---|---|
| A (list) | Play on where it was left. | |
| Y (list) | The book's bookmarks. | |
| START (list) | Start Over, or Mark as Finished / Unfinished. | |
| A | Pause or play. | |
| Left / Right | Jump back / on, as far as Jump Length in Music Settings says (10, 15 or 30 seconds). | Seek back / on. |
| L1 / R1 | Previous / next chapter. Previous starts the chapter over after its first second. | |
| L2 / R2 | Jump back / on, as Left / Right. | Keep jumping. |
| X | The next speed: 0.75×, 1×, 1.25×, 1.5×, 1.75×, 2×. | |
| Y | Bookmark the moment: a tick marks it on the bar. | The bookmarks. |
| SELECT | The chapters, to play from. | |
| START | Back to where you were before a skip, a chapter or a bookmark moved you; Chapters; Bookmarks (A plays from one, X takes it out); Speed; Sleep Timer (with End of Chapter); Mark as Finished. | |

With the screen off, L1 / R1 jump back / on in a book rather than changing chapter.

## Screen off

With `screen_off = play`, the shoulders still work with the lid closed:

| Button | Tapped | Held |
|---|---|---|
| L1 / R1 | Previous / next song; in an audiobook, jump back / on. | |
| R2 | Pause or play. | Fade out and stop, keeping the queue. |

Paused or stopped, slot powers off after its screen-off time.

## Settings

Theme follows slot's Appearance (Slot), or keeps the music screens Dark or Light. The clock and Up Next are set in `Config/music.txt`.

Hold A on Rebuild Library to read every song again. Update Library reads only what changed.
