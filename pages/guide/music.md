---
title: Apple Music
description: Read playlists and tracks, get listening statistics from the library's play counters, and build playlists through reviewed plans.
order: 6
---

MacFly reads your Apple Music library through the Music app's scripting
interface. It sees what is **in your library** (including Apple Music titles
you added), not the streaming catalog.

## Playlists and tracks

```bash
macfly music playlists                       # name, type (playlist · smart · folder), track count
macfly music tracks --limit 20               # library tracks with their persistent ID
macfly music tracks --playlist "Roadtrip"    # tracks of one playlist
```

The `id` column is the track's persistent ID. Plans use it to add tracks to a
playlist.

## Listening statistics

```bash
macfly music stats
macfly music stats --top 10 --stale-days 180
macfly music stats --csv ~/Desktop/music.csv
macfly music stats --format json
```

`music stats` evaluates the counters Music keeps for every track and prints:

- **Summary:** tracks, total plays, approximate listening hours (plays × duration),
  skips, never-played tracks, tracks heard in the last 30 days
- **Top lists:** tracks, artists, albums and genres by play count
- **Most skipped** tracks
- **Rediscover:** tracks you played before but not for `--stale-days` days (default 365)
- **Never played:** oldest additions first

`--csv` also writes every track with its counters (`played_count`,
`skipped_count`, `last_played`, `added_at`, `rating`, `duration_secs`) for
Numbers or Excel. `--format json` gives the same lists as structured data, and
agents get them over MCP as `music_stats`.

> Music stores **counters, not a listening history**. There is no record of
> individual plays with timestamps, so statistics per day, week or time of day
> are not possible. Plays from other devices only show up if your library syncs
> through iCloud.

## Building playlists

Writing goes through a plan like every other change:

```bash
macfly music add --playlist "Rediscovered" 6F6FF4727005BF1E A62BA311354E332D
```

This creates a `music_playlist_add` operation: the playlist is created if it
does not exist, and the tracks are added from your library. It is a
low-risk, additive change, so the default `balanced` preset applies it without
asking; under `strict` you approve it with `macfly plan approve <plan-id>`.

The typical agent flow combines both sides: "Find songs I played a lot but have
not heard in a year and put them into a playlist" → `music_stats` →
`plan_create` with `music_playlist_add`.

## Limits

- Smart playlists can be read but not created; Music does not let scripts set their rules.
- Tracks from the Apple Music catalog that are not in your library cannot be added.
- Removing tracks and reordering playlists are not supported.
