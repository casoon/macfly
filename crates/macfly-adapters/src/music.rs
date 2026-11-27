use macfly_core::error::MacflyError;
use macfly_core::plan::PlanOperation;
use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::jxa::run_jxa;

const SCRIPT_PLAYLISTS: &str = include_str!("../scripts/music_playlists.js");
const SCRIPT_TRACKS: &str = include_str!("../scripts/music_tracks.js");
const SCRIPT_STATS: &str = include_str!("../scripts/music_stats.js");
const SCRIPT_MUTATE: &str = include_str!("../scripts/music_mutate.js");

#[derive(Debug, Serialize, Deserialize)]
pub struct MusicPlaylist {
    pub name: String,
    /// `playlist`, `smart` oder `folder`
    pub kind: String,
    pub track_count: usize,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MusicTrack {
    pub id: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub genre: String,
    pub year: u32,
}

#[derive(Debug, Serialize)]
pub struct MusicPlaylistsResult {
    pub schema_version: String,
    pub total_matched: usize,
    pub playlists: Vec<MusicPlaylist>,
}

#[derive(Debug, Serialize)]
pub struct MusicTracksResult {
    pub schema_version: String,
    pub total_matched: usize,
    pub returned: usize,
    pub tracks: Vec<MusicTrack>,
}

/// Zähler pro Titel, so wie Music sie führt (kein Hörverlauf mit Zeitpunkten).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MusicTrackStat {
    pub id: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub genre: String,
    pub played_count: u64,
    pub skipped_count: u64,
    pub last_played: Option<String>,
    pub days_since_played: Option<u64>,
    pub added_at: Option<String>,
    /// 0–100 (Sterne × 20)
    pub rating: u32,
    pub duration_secs: u64,
}

#[derive(Debug, Serialize)]
pub struct MusicSummary {
    pub tracks: usize,
    pub total_plays: u64,
    pub total_skips: u64,
    /// Summe aus Wiedergaben × Dauer, in Stunden (Näherung)
    pub listened_hours: f64,
    pub never_played: usize,
    pub played_last_30_days: usize,
}

#[derive(Debug, Serialize)]
pub struct MusicGroupStat {
    pub name: String,
    pub plays: u64,
    pub skips: u64,
    pub tracks: usize,
}

#[derive(Debug, Serialize)]
pub struct MusicStatsResult {
    pub schema_version: String,
    pub summary: MusicSummary,
    pub top_tracks: Vec<MusicTrackStat>,
    pub top_artists: Vec<MusicGroupStat>,
    pub top_albums: Vec<MusicGroupStat>,
    pub top_genres: Vec<MusicGroupStat>,
    pub most_skipped: Vec<MusicTrackStat>,
    /// Früher gehört (≥ 1 Wiedergabe), aber seit `stale_days` Tagen nicht mehr.
    pub stale: Vec<MusicTrackStat>,
    /// Zufällig/älteste zuerst: nie gespielte Titel, älteste Zugänge zuerst.
    pub never_played_oldest: Vec<MusicTrackStat>,
    #[serde(skip)]
    pub all: Vec<MusicTrackStat>,
}

#[derive(Debug, Deserialize)]
struct RawStats {
    tracks: Vec<MusicTrackStat>,
}

fn group_by(
    tracks: &[MusicTrackStat],
    key: impl Fn(&MusicTrackStat) -> String,
    top: usize,
) -> Vec<MusicGroupStat> {
    let mut map: HashMap<String, MusicGroupStat> = HashMap::new();
    for t in tracks {
        let k = key(t);
        if k.is_empty() {
            continue;
        }
        let e = map.entry(k.clone()).or_insert(MusicGroupStat {
            name: k,
            plays: 0,
            skips: 0,
            tracks: 0,
        });
        e.plays += t.played_count;
        e.skips += t.skipped_count;
        e.tracks += 1;
    }
    let mut v: Vec<_> = map.into_values().filter(|g| g.plays > 0).collect();
    v.sort_by(|a, b| b.plays.cmp(&a.plays).then(a.name.cmp(&b.name)));
    v.truncate(top);
    v
}

#[derive(Debug, Deserialize)]
struct RawPlaylists {
    total_matched: usize,
    playlists: Vec<MusicPlaylist>,
}

#[derive(Debug, Deserialize)]
struct RawTracks {
    total_matched: usize,
    tracks: Vec<MusicTrack>,
}

#[derive(Debug, Deserialize)]
struct RawMutateResponse {
    ok: bool,
    detail: String,
}

pub struct MusicAdapter {
    timeout_secs: u64,
}

impl MusicAdapter {
    pub fn new(timeout_secs: u64) -> Self {
        Self { timeout_secs }
    }

    pub fn playlists(&self) -> Result<MusicPlaylistsResult, MacflyError> {
        let raw: RawPlaylists = run_jxa(
            "Apple Music",
            SCRIPT_PLAYLISTS,
            &serde_json::json!({}),
            self.timeout_secs,
        )?;
        Ok(MusicPlaylistsResult {
            schema_version: "1.0".to_string(),
            total_matched: raw.total_matched,
            playlists: raw.playlists,
        })
    }

    pub fn tracks(
        &self,
        playlist: Option<&str>,
        limit: usize,
    ) -> Result<MusicTracksResult, MacflyError> {
        let payload = serde_json::json!({ "playlist": playlist, "limit": limit });
        let raw: RawTracks = run_jxa("Apple Music", SCRIPT_TRACKS, &payload, self.timeout_secs)?;
        Ok(MusicTracksResult {
            schema_version: "1.0".to_string(),
            total_matched: raw.total_matched,
            returned: raw.tracks.len(),
            tracks: raw.tracks,
        })
    }

    /// Liest die Zähler aller Titel und wertet sie aus.
    pub fn stats(&self, top: usize, stale_days: u64) -> Result<MusicStatsResult, MacflyError> {
        let raw: RawStats = run_jxa(
            "Apple Music",
            SCRIPT_STATS,
            &serde_json::json!({}),
            self.timeout_secs.max(120),
        )?;
        let all = raw.tracks;

        let by_plays = |a: &MusicTrackStat, b: &MusicTrackStat| {
            b.played_count
                .cmp(&a.played_count)
                .then(a.artist.cmp(&b.artist))
                .then(a.title.cmp(&b.title))
        };
        let mut top_tracks: Vec<_> = all.iter().filter(|t| t.played_count > 0).cloned().collect();
        top_tracks.sort_by(by_plays);
        top_tracks.truncate(top);

        let mut most_skipped: Vec<_> = all
            .iter()
            .filter(|t| t.skipped_count > 0)
            .cloned()
            .collect();
        most_skipped.sort_by(|a, b| {
            b.skipped_count
                .cmp(&a.skipped_count)
                .then(a.title.cmp(&b.title))
        });
        most_skipped.truncate(top);

        let mut stale: Vec<_> = all
            .iter()
            .filter(|t| t.played_count > 0 && t.days_since_played.is_some_and(|d| d >= stale_days))
            .cloned()
            .collect();
        stale.sort_by(|a, b| {
            b.played_count
                .cmp(&a.played_count)
                .then(b.days_since_played.cmp(&a.days_since_played))
        });
        stale.truncate(top);

        let mut never: Vec<_> = all
            .iter()
            .filter(|t| t.played_count == 0)
            .cloned()
            .collect();
        never.sort_by(|a, b| a.added_at.cmp(&b.added_at));
        let never_count = never.len();
        never.truncate(top);

        let total_plays: u64 = all.iter().map(|t| t.played_count).sum();
        let secs: u64 = all.iter().map(|t| t.played_count * t.duration_secs).sum();
        let summary = MusicSummary {
            tracks: all.len(),
            total_plays,
            total_skips: all.iter().map(|t| t.skipped_count).sum(),
            listened_hours: (secs as f64 / 3600.0 * 10.0).round() / 10.0,
            never_played: never_count,
            played_last_30_days: all
                .iter()
                .filter(|t| t.days_since_played.is_some_and(|d| d < 30))
                .count(),
        };

        Ok(MusicStatsResult {
            schema_version: "1.0".to_string(),
            summary,
            top_artists: group_by(&all, |t| t.artist.clone(), top),
            top_albums: group_by(&all, |t| format!("{} – {}", t.artist, t.album), top),
            top_genres: group_by(&all, |t| t.genre.clone(), top),
            top_tracks,
            most_skipped,
            stale,
            never_played_oldest: never,
            all,
        })
    }

    pub fn execute_operation(&self, op: &PlanOperation) -> Result<String, MacflyError> {
        let payload = match op {
            PlanOperation::MusicPlaylistAdd {
                playlist,
                track_ids,
            } => serde_json::json!({
                "action": "music_playlist_add",
                "playlist": playlist,
                "track_ids": track_ids,
            }),
            _ => {
                return Err(MacflyError::PlanError(
                    "Operation gehört nicht zum Music-Adapter".to_string(),
                ));
            }
        };
        let res: RawMutateResponse =
            run_jxa("Apple Music", SCRIPT_MUTATE, &payload, self.timeout_secs)?;
        if !res.ok {
            return Err(MacflyError::ScriptError(res.detail));
        }
        Ok(res.detail)
    }
}
