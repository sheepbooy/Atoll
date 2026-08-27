//! Timed lyrics via the free LRCLIB API (https://lrclib.net).
//!
//! The MediaRemote adapter cannot provide lyrics, so we fetch timed LRC
//! lyrics from LRCLIB on track change and track the current line locally.

use base64::Engine as _;
use serde::{Deserialize, Serialize};

/// A single synced lyric line.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LyricLine {
    pub time_ms: u64,
    pub text: String,
}

/// Full lyrics payload emitted to the frontend.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LyricPayload {
    pub lines: Vec<LyricLine>,
    pub current_index: usize,
    pub next_time_ms: Option<u64>,
    pub track_title: Option<String>,
    pub track_artist: Option<String>,
}

const LRCLIB_GET: &str = "https://lrclib.net/api/get";
const LRCLIB_SEARCH: &str = "https://lrclib.net/api/search";

/// Raw LRCLIB `/api/get` response.
#[derive(Deserialize)]
struct LrclibResponse {
    #[serde(rename = "syncedLyrics")]
    synced_lyrics: Option<String>,
}

/// A single `/api/search` result entry.
#[derive(Deserialize)]
struct LrclibSearchEntry {
    #[serde(rename = "syncedLyrics")]
    synced_lyrics: Option<String>,
    duration: Option<f64>,
}

/// Fetch synced LRC lyrics for a track. Returns None if no synced lyrics exist.
///
/// Fallback chain (Chinese-first, best coverage for QQ Music / NetEase /
/// KuGou catalogs, then LRCLIB for international tracks):
/// 1. NetEase Music — search by artist+title, verify title/artist/duration.
/// 2. QQ Music — search by artist+title, verify title/artist/duration.
/// 3. LRCLIB `/api/get` then `/api/search` — international catalog.
///
/// The search APIs rank loosely: top hits may be other songs by the same
/// artist, covers, or different cuts (live / DJ / 伴奏) of the requested
/// track. Every candidate is verified before use — wrong lyrics desync the
/// marquee worse than no lyrics at all.
pub fn fetch_lyrics(
    artist: &str,
    title: &str,
    album: Option<&str>,
    duration: Option<f64>,
) -> Option<Vec<LyricLine>> {
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(8))
        .build()
        .ok()?;

    // 1. NetEase Music (primary — best Chinese coverage).
    if let Some(lines) = fetch_netease(&client, artist, title, duration) {
        return Some(lines);
    }

    // 2. QQ Music fallback.
    if let Some(lines) = fetch_qq(&client, artist, title, duration) {
        return Some(lines);
    }

    // 3. LRCLIB fallback (international catalog).
    if let Some(lines) = fetch_get(&client, artist, title, album, duration) {
        return Some(lines);
    }
    fetch_search(&client, artist, title, duration)
}

/// A duration this far from the playing track means the candidate is a
/// different cut (live / DJ / instrumental) and must be rejected.
const DURATION_TOLERANCE_SECS: f64 = 10.0;

/// Normalize a title/artist for matching: lowercase, drop whitespace
/// (including U+200B), and unify full-width punctuation so "歌名（注释）"
/// and "歌名(注释)" compare equal.
fn normalize_for_match(raw: &str) -> String {
    raw.to_lowercase()
        .chars()
        .map(|c| match c {
            '（' => '(',
            '）' => ')',
            '【' => '[',
            '】' => ']',
            '，' => ',',
            '；' => ';',
            _ => c,
        })
        .filter(|c| !c.is_whitespace() && *c != '\u{200b}')
        .collect()
}

/// Split an artist string billed as "A/B", "A、B", "A & B" into parts.
fn split_artists(raw: &str) -> Vec<String> {
    raw.split(|c: char| matches!(c, '/' | '&' | ',' | ';' | '、' | '，' | '；'))
        .map(normalize_for_match)
        .filter(|s| !s.is_empty())
        .collect()
}

/// True when a target artist part overlaps a candidate artist part
/// (containment both ways, so billing differences like "金志文" vs
/// "金志文/汤晓菲" still match). The contained side must be ≥2 chars to
/// avoid single-character false positives.
fn artist_matches(target: &str, candidates: &[String]) -> bool {
    let targets = split_artists(target);
    if targets.is_empty() {
        return false;
    }
    let cands: Vec<String> = candidates.iter().flat_map(|c| split_artists(c)).collect();
    targets.iter().any(|t| {
        cands.iter().any(|c| {
            (c.contains(t.as_str()) && t.chars().count() >= 2)
                || (t.contains(c.as_str()) && c.chars().count() >= 2)
        })
    })
}

/// Title match quality: 0 = identical after normalization, 1 = one side is
/// the other plus an annotation ("歌名" vs "歌名 (Live版)" — the duration
/// gate stays the authority on which version), None = different titles.
fn title_match_tier(target: &str, candidate: &str) -> Option<u8> {
    let t = normalize_for_match(target);
    let c = normalize_for_match(candidate);
    if t.is_empty() || c.is_empty() {
        return None;
    }
    if t == c {
        return Some(0);
    }
    if c.contains(&t) || t.contains(&c) {
        return Some(1);
    }
    None
}

/// A search-result candidate under consideration.
struct LyricsCandidate {
    title: String,
    artists: Vec<String>,
    duration_secs: Option<f64>,
}

impl LyricsCandidate {
    /// Some((title_tier, duration_diff_ms)) when this candidate is a
    /// credible match (lower is better), None when it must be rejected.
    fn score(
        &self,
        target_title: &str,
        target_artist: &str,
        target_duration: Option<f64>,
    ) -> Option<(u8, u64)> {
        let tier = title_match_tier(target_title, &self.title)?;
        if !artist_matches(target_artist, &self.artists) {
            return None;
        }
        let diff_ms = match (target_duration, self.duration_secs) {
            (Some(t), Some(c)) => {
                let diff = (t - c).abs();
                if diff > DURATION_TOLERANCE_SECS {
                    return None;
                }
                (diff * 1000.0) as u64
            }
            _ => 0,
        };
        Some((tier, diff_ms))
    }
}

/// Pick the best (lowest-scored) candidate; None when none is credible.
fn pick_best_index(
    candidates: &[LyricsCandidate],
    target_title: &str,
    target_artist: &str,
    target_duration: Option<f64>,
) -> Option<usize> {
    let mut best: Option<((u8, u64), usize)> = None;
    for (i, c) in candidates.iter().enumerate() {
        let Some(score) = c.score(target_title, target_artist, target_duration) else {
            continue;
        };
        if best.map_or(true, |(b, _)| score < b) {
            best = Some((score, i));
        }
    }
    best.map(|(_, i)| i)
}

/// NetEase search response.
#[derive(Deserialize)]
struct NetEaseSearchResult {
    result: Option<NetEaseResult>,
}

#[derive(Deserialize)]
struct NetEaseResult {
    songs: Option<Vec<NetEaseSong>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct NetEaseSong {
    id: u64,
    name: String,
    duration: Option<u64>,
    artists: Option<Vec<NetEaseArtist>>,
}

#[derive(Deserialize)]
struct NetEaseArtist {
    name: String,
}

/// NetEase lyric response.
#[derive(Deserialize)]
struct NetEaseLyricResponse {
    lrc: Option<NetEaseLyricBody>,
}

#[derive(Deserialize)]
struct NetEaseLyricBody {
    lyric: Option<String>,
}

fn fetch_netease(
    client: &reqwest::blocking::Client,
    artist: &str,
    title: &str,
    duration: Option<f64>,
) -> Option<Vec<LyricLine>> {
    let query = format!("{} {}", artist, title);
    let resp = client
        .get("https://music.163.com/api/search/get")
        .query(&[("s", query.as_str()), ("type", "1"), ("limit", "5")])
        .send()
        .ok()?;
    if !resp.status().is_success() {
        return None;
    }
    let search: NetEaseSearchResult = resp.json().ok()?;
    let songs = search.result?.songs?;
    let candidates: Vec<LyricsCandidate> = songs
        .iter()
        .map(|s| LyricsCandidate {
            title: s.name.clone(),
            artists: s
                .artists
                .as_ref()
                .map(|a| a.iter().map(|x| x.name.clone()).collect())
                .unwrap_or_default(),
            duration_secs: s.duration.map(|ms| ms as f64 / 1000.0),
        })
        .collect();
    let index = pick_best_index(&candidates, title, artist, duration)?;
    let song = &songs[index];
    // Fetch lyrics by song id.
    let lyric_url = format!("https://music.163.com/api/song/lyric?id={}&lv=1", song.id);
    let resp = client.get(&lyric_url).send().ok()?;
    if !resp.status().is_success() {
        return None;
    }
    let body: NetEaseLyricResponse = resp.json().ok()?;
    body.lrc
        .and_then(|l| l.lyric)
        .map(|raw| filter_metadata(parse_lrc(&raw)))
        .filter(|lines| !lines.is_empty())
}

/// QQ Music search response.
#[derive(Deserialize)]
struct QqSearchResponse {
    data: Option<QqSearchData>,
}

#[derive(Deserialize)]
struct QqSearchData {
    song: Option<QqSongList>,
}

#[derive(Deserialize)]
struct QqSongList {
    list: Option<Vec<QqSong>>,
}

#[derive(Deserialize)]
struct QqSong {
    // NOTE: the search API returns all-lowercase keys ("songmid"/"songname"),
    // not camelCase — a rename_all here silently breaks deserialization and
    // disables the whole QQ source.
    #[serde(rename = "songmid")]
    song_mid: String,
    #[serde(rename = "songname")]
    song_name: String,
    singer: Option<Vec<QqSinger>>,
    interval: Option<u64>,
}

#[derive(Deserialize)]
struct QqSinger {
    name: String,
}

/// QQ Music lyric response (lyric is base64-encoded).
#[derive(Deserialize)]
struct QqLyricResponse {
    lyric: Option<String>,
}

fn fetch_qq(
    client: &reqwest::blocking::Client,
    artist: &str,
    title: &str,
    duration: Option<f64>,
) -> Option<Vec<LyricLine>> {
    let query = format!("{} {}", artist, title);
    let resp = client
        .get("https://c.y.qq.com/soso/fcgi-bin/client_search_cp")
        .query(&[("p", "1"), ("n", "5"), ("w", query.as_str()), ("format", "json")])
        .header("Referer", "https://y.qq.com")
        .send()
        .ok()?;
    if !resp.status().is_success() {
        return None;
    }
    let search: QqSearchResponse = resp.json().ok()?;
    let songs = search.data?.song?.list?;
    let candidates: Vec<LyricsCandidate> = songs
        .iter()
        .map(|s| LyricsCandidate {
            title: s.song_name.clone(),
            artists: s
                .singer
                .as_ref()
                .map(|v| v.iter().map(|x| x.name.clone()).collect())
                .unwrap_or_default(),
            duration_secs: s.interval.map(|sec| sec as f64),
        })
        .collect();
    let index = pick_best_index(&candidates, title, artist, duration)?;
    let song = &songs[index];
    // Fetch lyrics by songmid.
    let lyric_url = format!(
        "https://c.y.qq.com/lyric/fcgi-bin/fcg_query_lyric_new.fcg?songmid={}&pcachetime=1&format=json",
        song.song_mid
    );
    let resp = client
        .get(&lyric_url)
        .header("Referer", "https://y.qq.com")
        .send()
        .ok()?;
    if !resp.status().is_success() {
        return None;
    }
    let body: QqLyricResponse = resp.json().ok()?;
    body.lyric
        .and_then(|b64| base64::engine::general_purpose::STANDARD.decode(b64).ok())
        .and_then(|bytes| String::from_utf8(bytes).ok())
        .map(|raw| filter_metadata(parse_lrc(&raw)))
        .filter(|lines| !lines.is_empty())
}

/// Remove metadata rows (作词/作曲/编曲 etc.) that have timestamps but
/// aren't actual lyric lines.
fn filter_metadata(mut lines: Vec<LyricLine>) -> Vec<LyricLine> {
    lines.retain(|l| {
        let t = l.text.as_str();
        !t.contains("作词") && !t.contains("作曲") && !t.contains("编曲")
            && !t.contains("制作人") && !t.contains("混音") && !t.contains("录音")
            && !t.contains("和声") && !t.contains("母带") && !t.contains("词：")
            && !t.contains("曲：")
    });
    lines
}

fn fetch_get(
    client: &reqwest::blocking::Client,
    artist: &str,
    title: &str,
    album: Option<&str>,
    duration: Option<f64>,
) -> Option<Vec<LyricLine>> {
    let mut req = client
        .get(LRCLIB_GET)
        .query(&[("artist_name", artist), ("track_name", title)]);
    if let Some(al) = album {
        req = req.query(&[("album_name", al)]);
    }
    let duration_str;
    if let Some(d) = duration {
        duration_str = (d.round() as i64).to_string();
        req = req.query(&[("duration", duration_str.as_str())]);
    }
    let resp = req.send().ok()?;
    if !resp.status().is_success() {
        return None;
    }
    let body: LrclibResponse = resp.json().ok()?;
    body.synced_lyrics
        .as_deref()
        .map(parse_lrc)
        .filter(|lines| !lines.is_empty())
}

fn fetch_search(
    client: &reqwest::blocking::Client,
    artist: &str,
    title: &str,
    duration: Option<f64>,
) -> Option<Vec<LyricLine>> {
    let resp = client
        .get(LRCLIB_SEARCH)
        .query(&[("artist_name", artist), ("track_name", title)])
        .send()
        .ok()?;
    if !resp.status().is_success() {
        return None;
    }
    let entries: Vec<LrclibSearchEntry> = resp.json().ok()?;
    // Prefer entries with synced lyrics closest to the reported duration.
    let target = duration.unwrap_or(0.0);
    entries
        .into_iter()
        .filter(|e| e.synced_lyrics.is_some())
        .min_by_key(|e| {
            let d = e.duration.unwrap_or(0.0);
            ((d - target).abs() * 1000.0) as u64
        })
        .and_then(|e| e.synced_lyrics.as_deref().map(parse_lrc))
        .filter(|lines| !lines.is_empty())
}

/// Parse LRC text (`[mm:ss.xx]text`) into sorted `LyricLine` entries.
/// Handles multiple timestamps per line and optional centiseconds.
pub fn parse_lrc(raw: &str) -> Vec<LyricLine> {
    let mut out: Vec<LyricLine> = Vec::new();
    for line in raw.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        // Collect all leading `[mm:ss.xx]` or `[mm:ss]` tags on this line.
        let mut rest = trimmed;
        let mut times: Vec<u64> = Vec::new();
        loop {
            let Some(close) = rest.find(']') else { break };
            let tag = &rest[..close + 1];
            if let Some(ms) = parse_lrc_time(tag) {
                times.push(ms);
            } else {
                break; // not a timestamp tag — stop
            }
            rest = &rest[close + 1..];
        }
        if times.is_empty() {
            continue;
        }
        let text = rest.trim().to_string();
        for ms in times {
            out.push(LyricLine { time_ms: ms, text: text.clone() });
        }
    }
    out.sort_by_key(|l| l.time_ms);
    // Remove duplicate timestamps (keep first).
    out.dedup_by_key(|l| l.time_ms);
    out
}

/// Parse `[mm:ss.xx]` or `[mm:ss]` into milliseconds.
fn parse_lrc_time(tag: &str) -> Option<u64> {
    let inner = tag.strip_prefix('[')?.strip_suffix(']')?;
    let mut parts = inner.splitn(2, ':');
    let mins: u64 = parts.next()?.parse().ok()?;
    let rest = parts.next()?;
    // rest may be "ss.xx", "ss.xx0", or just "ss"
    if let Some((secs_str, frac_str)) = rest.split_once('.') {
        let secs: u64 = secs_str.parse().ok()?;
        // Pad/truncate fraction to 3 digits (ms).
        let ms: u64 = format!("{:0<3}", frac_str.get(..3).unwrap_or(frac_str))
            .parse()
            .unwrap_or(0);
        Some(mins * 60_000 + secs * 1000 + ms)
    } else {
        let secs: u64 = rest.parse().ok()?;
        Some(mins * 60_000 + secs * 1000)
    }
}

/// Find the index of the line active at `position_sec` (binary search).
/// Returns 0 if position is before the first line.
pub fn current_line_index(lines: &[LyricLine], position_sec: f64) -> usize {
    let pos_ms = (position_sec * 1000.0) as u64;
    match lines.binary_search_by_key(&pos_ms, |l| l.time_ms) {
        Ok(i) => i,
        Err(i) => i.saturating_sub(1),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_simple_lrc() {
        let raw = "[00:01.00]Hello\n[00:03.50]World\n[00:05.00]End";
        let lines = parse_lrc(raw);
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[0].time_ms, 1000);
        assert_eq!(lines[0].text, "Hello");
        assert_eq!(lines[1].time_ms, 3500);
        assert_eq!(lines[2].time_ms, 5000);
    }

    #[test]
    fn parses_multi_timestamp_line() {
        let raw = "[00:01.00][00:03.00]Repeat";
        let lines = parse_lrc(raw);
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].time_ms, 1000);
        assert_eq!(lines[1].time_ms, 3000);
        assert_eq!(lines[0].text, "Repeat");
    }

    #[test]
    fn parses_without_centiseconds() {
        let raw = "[00:10]No fractional";
        let lines = parse_lrc(raw);
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].time_ms, 10_000);
    }

    #[test]
    fn skips_non_timestamp_lines() {
        let raw = "[ti:Song Title]\n[00:01.00]First\nSome plain text";
        let lines = parse_lrc(raw);
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].text, "First");
    }

    #[test]
    fn sorts_unordered_lines() {
        let raw = "[00:05.00]B\n[00:01.00]A";
        let lines = parse_lrc(raw);
        assert_eq!(lines[0].time_ms, 1000);
        assert_eq!(lines[1].time_ms, 5000);
    }

    #[test]
    fn current_line_at_start() {
        let lines = parse_lrc("[00:01.00]A\n[00:03.00]B\n[00:05.00]C");
        assert_eq!(current_line_index(&lines, 0.0), 0);
        assert_eq!(current_line_index(&lines, 0.5), 0);
        assert_eq!(current_line_index(&lines, 1.0), 0);
        assert_eq!(current_line_index(&lines, 2.0), 0);
        assert_eq!(current_line_index(&lines, 3.0), 1);
        assert_eq!(current_line_index(&lines, 4.0), 1);
        assert_eq!(current_line_index(&lines, 5.0), 2);
        assert_eq!(current_line_index(&lines, 99.0), 2);
    }

    #[test]
    fn empty_lines_yield_zero() {
        let lines: Vec<LyricLine> = vec![];
        assert_eq!(current_line_index(&lines, 10.0), 0);
    }

    #[test]
    fn normalize_unifies_width_case_and_space() {
        assert_eq!(normalize_for_match("肩上蝶（我愿意） FEAT"), "肩上蝶(我愿意)feat");
        assert_eq!(normalize_for_match("A　B"), "ab");
    }

    #[test]
    fn title_match_tiers_identical_annotation_and_reject() {
        // Same title after normalization (full/half-width punctuation).
        assert_eq!(title_match_tier("肩上蝶（我愿意化蝶在你肩上落）", "肩上蝶 (我愿意化蝶在你肩上落)"), Some(0));
        // Clean title vs annotated variant of the same song — tier 1; the
        // duration gate decides whether the version is acceptable.
        assert_eq!(title_match_tier("肩上蝶", "肩上蝶 (Live版)"), Some(1));
        // Different annotations / different songs are different titles.
        assert_eq!(title_match_tier("肩上蝶（我愿意化蝶在你肩上落）", "肩上蝶 (Live版)"), None);
        assert_eq!(title_match_tier("是龙", "着魔"), None);
        assert_eq!(title_match_tier("肩上蝶", ""), None);
    }

    #[test]
    fn artist_matches_billing_variations() {
        assert!(artist_matches("金志文", &["金志文".into(), "汤晓菲".into()]));
        assert!(artist_matches("金志文/汤晓菲", &["金志文".into()]));
        assert!(!artist_matches("张杰", &["石龙".into()]));
        assert!(!artist_matches("张杰", &[]));
    }

    #[test]
    fn candidate_scoring_rejects_wrong_song_or_version() {
        // "是龙" by 张杰 (233s): NetEase's top hits are a cover by 石龙 and
        // other songs by 张杰 — all must be rejected.
        let assert_reject = |title: &str, artists: &[&str], dur: Option<f64>| {
            let artists: Vec<String> = artists.iter().map(|s| s.to_string()).collect();
            let cand = LyricsCandidate {
                title: title.to_string(),
                artists,
                duration_secs: dur,
            };
            assert!(cand.score("是龙", "张杰", Some(233.0)).is_none());
        };
        assert_reject("是龙", &["石龙"], Some(234.0));
        assert_reject("着魔", &["张杰"], Some(215.0));
        assert_reject("是龙 (DJ 晓遥版)", &["张杰"], Some(187.0));

        // Live cut 26s longer than the playing studio version.
        let live = LyricsCandidate {
            title: "肩上蝶".to_string(),
            artists: vec!["金志文".into(), "陈冰".into()],
            duration_secs: Some(282.0),
        };
        assert!(live.score("肩上蝶", "金志文", Some(256.0)).is_none());

        // Among credible candidates the closest duration wins.
        let exact = LyricsCandidate {
            title: "是龙".to_string(),
            artists: vec!["张杰".into()],
            duration_secs: Some(233.0),
        };
        let close = LyricsCandidate {
            title: "是龙".to_string(),
            artists: vec!["张杰".into()],
            duration_secs: Some(230.0),
        };
        assert!(exact.score("是龙", "张杰", Some(233.0)).unwrap() < close.score("是龙", "张杰", Some(233.0)).unwrap());

        // Unknown durations are acceptable (tier + zero diff).
        let unknown = LyricsCandidate {
            title: "是龙".to_string(),
            artists: vec!["张杰".into()],
            duration_secs: None,
        };
        assert_eq!(unknown.score("是龙", "张杰", None), Some((0, 0)));
    }

    #[test]
    fn pick_best_prefers_exact_duration_match() {
        let candidates = vec![
            LyricsCandidate {
                title: "看我72变".to_string(),
                artists: vec!["蔡依林".into()],
                duration_secs: Some(224.0),
            },
            LyricsCandidate {
                title: "看我72变".to_string(),
                artists: vec!["蔡依林".into()],
                duration_secs: Some(226.0),
            },
        ];
        assert_eq!(pick_best_index(&candidates, "看我72变", "蔡依林", Some(226.0)), Some(1));
        // No credible candidate at all.
        let bad = vec![LyricsCandidate {
            title: "着魔".to_string(),
            artists: vec!["张杰".into()],
            duration_secs: Some(215.0),
        }];
        assert_eq!(pick_best_index(&bad, "是龙", "张杰", Some(233.0)), None);
    }

    #[test]
    fn pick_best_prefers_exact_title_over_annotated_variant() {
        // An exact-title candidate wins even when an annotated variant
        // (same base title, e.g. a karaoke cut) is closer in duration.
        let candidates = vec![
            LyricsCandidate {
                title: "肩上蝶 (哪怕爱跌落了还不如昨) (伴奏)".to_string(),
                artists: vec!["金志文".into(), "苏美璇".into()],
                duration_secs: Some(257.0),
            },
            LyricsCandidate {
                title: "肩上蝶".to_string(),
                artists: vec!["金志文".into()],
                duration_secs: Some(253.0),
            },
        ];
        assert_eq!(pick_best_index(&candidates, "肩上蝶", "金志文", Some(256.0)), Some(1));
    }

    /// Live integration test for NetEase (primary source) — requires network.
    ///   cargo test --lib lyrics::tests::fetch_live_netease -- --ignored
    #[test]
    #[ignore]
    fn fetch_live_netease() {
        // "告别前要跳舞" — NetEase has the correct version.
        let lines = fetch_lyrics("汪苏泷", "告别前要跳舞", None, Some(279.0));
        assert!(lines.is_some(), "expected synced lyrics");
        let l = lines.unwrap();
        assert!(!l.is_empty());
        // Verify it's the RIGHT lyrics (not a wrong cover).
        assert!(l.iter().any(|x| x.text.contains("浮夸") || x.text.contains("遁入")));
        println!("告别前要跳舞: got {} lines", l.len());
    }

    /// Live integration test for QQ Music fallback — requires network.
    ///   cargo test --lib lyrics::tests::fetch_live_qq -- --ignored
    #[test]
    #[ignore]
    fn fetch_live_qq() {
        // "闪电" — QQ Music should have it even if NetEase doesn't.
        let lines = fetch_lyrics("汪苏泷", "闪电", None, Some(260.0));
        assert!(lines.is_some(), "expected synced lyrics");
        let l = lines.unwrap();
        assert!(!l.is_empty());
        assert!(l.iter().all(|x| !x.text.contains("作词") && !x.text.contains("作曲")));
        println!("闪电: got {} lines", l.len());
    }

    /// Live integration test for LRCLIB fallback (international) — requires network.
    ///   cargo test --lib lyrics::tests::fetch_live_lrclib -- --ignored
    #[test]
    #[ignore]
    fn fetch_live_lrclib() {
        // "有点甜" — available on all sources, tests the full chain.
        let lines = fetch_lyrics("汪苏泷", "有点甜", None, Some(235.0));
        assert!(lines.is_some());
        let l = lines.unwrap();
        assert!(!l.is_empty());
        println!("有点甜: got {} lines", l.len());
    }

    /// Live regression tests for candidate verification — require network.
    ///   cargo test --lib lyrics::tests::fetch_live_matching -- --ignored
    ///
    /// Both cases were real mis-fetches before title/duration verification
    /// was added to the NetEase/QQ sources.
    #[test]
    #[ignore]
    fn fetch_live_matching_rejects_wrong_song() {
        // NetEase's top hits for "张杰 是龙" are a cover by 石龙 and OTHER
        // songs by 张杰 (着魔/剑心/他不懂/天下) — the old artist-only check
        // matched 着魔 and displayed its lyrics for 是龙.
        let lines = fetch_lyrics("张杰", "是龙", None, Some(233.0));
        assert!(lines.is_some(), "expected synced lyrics");
        let l = lines.unwrap();
        assert!(l.iter().any(|x| x.text.contains("越战越勇") || x.text.contains("图腾")));
        assert!(l.iter().all(|x| !x.text.contains("心魔乱舞")));
        println!("是龙: got {} lines", l.len());
    }

    #[test]
    #[ignore]
    fn fetch_live_matching_rejects_wrong_version() {
        // The studio original (256s) is absent from NetEase's top 5 — the
        // old code accepted the 282s Live duet version and its lyrics.
        let lines = fetch_lyrics("金志文", "肩上蝶（我愿意化蝶在你肩上落）", None, Some(256.0));
        assert!(lines.is_some(), "expected synced lyrics");
        let l = lines.unwrap();
        assert!(l.iter().any(|x| x.text.contains("一首歌唱不完")));
        // Live-arrangement credits from the wrong version must not appear.
        assert!(l.iter().all(|x| !x.text.contains("维伴音乐")));
        println!("肩上蝶: got {} lines", l.len());
    }
}
