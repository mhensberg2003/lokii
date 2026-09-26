# Lokii

A desktop app (macOS, Windows) that finds anime, streams episodes through TorBox or a local torrent, and keeps all user state on the device.

## Language

### Catalog

**Show**:
One AniList media entry, or one One Pace Arc. Each season or cour is its own Show, linked to the others as sequel or prequel.
_Avoid_: Anime, series, title

**Episode**:
One numbered episode of a Show.
_Avoid_: Ep, chapter

**Related Show**:
A Show that AniList links to another Show (sequel, prequel, side story).

**Franchise**:
The chain of Shows linked by sequel and prequel links. The user sees each Show in a Franchise as a season on one page.
_Avoid_: Series, collection

**One Pace**:
A fan recut of the One Piece anime that follows the manga. It is not on AniList: Lokii builds it from the One Pace Episode Guide and shows it as one Franchise of Arcs.

**Arc**:
One story arc of One Pace ("Wano"). Each Arc is one Show; its Episodes are the One Pace Episodes of that arc.
_Avoid_: Saga (a group of Arcs)

### Finding video

**Index**:
A searchable list of Releases. AnimeTosho is the Index for AniList Shows; Nyaa is the Index for One Pace.
_Avoid_: Provider, tracker, source

**Release**:
One torrent from a release group that contains one or more Episodes of a Show.
_Avoid_: Torrent, upload, file

**Batch**:
A Release that contains many Episodes, often a full Show.

**Best Release**:
The Release that SeaDex recommends for a Show. For One Pace: the newest Release of the Episode Guide's current file, a single-Episode Release before a Batch.
_Avoid_: Recommended torrent

**Chosen Release**:
The Release the app plays for an Episode: the Best Release, else the 1080p Release with the most seeders, unless the user picked another one. A user's pick applies to the whole Show: for an Episode that the picked Release does not contain, the app uses a Release from the same group at the same resolution.
_Avoid_: Selected torrent, default release

### Playback

**Source**:
Where the video bytes of a Release come from: TorBox or Local Torrent.
_Avoid_: Provider, backend, debrid, Index

**TorBox**:
The Source used whenever the user has connected a TorBox account.

**Local Torrent**:
The Source used when TorBox is not connected. The device downloads and seeds the Release itself.

**Stream**:
One playback of one Episode from one Source.
_Avoid_: Session, playback job

**Skip Segment**:
A time range in an Episode (intro or outro) that the player offers to skip.
_Avoid_: Chapter, OP/ED marker

### Library

**Watch Progress**:
The last position the user reached in an Episode.
_Avoid_: History, resume point

**Watched**:
An Episode whose Watch Progress passed 90% of its length.
_Avoid_: Completed, seen

**Up Next**:
The Episode to play next for a Show: the last played Episode when the user left it before it was Watched, else the first Episode after the last Watched one. After the last Episode of a Show, it is Episode 1 of the next Show in the Franchise.

**Continue watching**:
The Up Next Episodes of the Shows the user played most recently.
_Avoid_: Recently watched, history

**Watchlist**:
Shows the user saved to watch later.
_Avoid_: My List, favorites, queue
