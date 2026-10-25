use base64::Engine;
use serde::Serialize;
use std::time::{SystemTime, UNIX_EPOCH};
use windows::Media::Control::{
    GlobalSystemMediaTransportControlsSessionManager as Manager,
    GlobalSystemMediaTransportControlsSessionMediaProperties as MediaProperties,
    GlobalSystemMediaTransportControlsSessionPlaybackStatus as Status,
};
use windows::Storage::Streams::{DataReader, IRandomAccessStreamReference};

#[derive(Serialize, Clone, PartialEq, Debug)]
#[serde(rename_all = "camelCase")]
pub struct MediaInfo {
    pub title: String,
    pub artist: String,
    pub album: String,
    pub app_id: String,
    pub playing: bool,
    pub position_ms: u64,
    pub duration_ms: u64,
    /// Album art as a data URL.
    pub thumbnail: Option<String>,
}

/// Album art is expensive to read, so keep it until the track changes.
#[derive(Default)]
pub struct ThumbCache {
    key: String,
    data: Option<String>,
    /// Polls left in which to re-read the art after a track change.
    rereads: u8,
}

/// Browsers publish the new title before the new artwork, so right after a
/// track change the session still hands out the previous song's cover.
/// Keep re-reading for a few polls (~4s) so the late artwork replaces it.
const REREADS_AFTER_CHANGE: u8 = 6;

pub fn manager() -> windows::core::Result<Manager> {
    Manager::RequestAsync()?.get()
}

// 100ns ticks between 1601-01-01 (WinRT DateTime epoch) and 1970-01-01.
const EPOCH_DIFF_TICKS: i64 = 116_444_736_000_000_000;

fn now_ticks() -> i64 {
    let d = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default();
    d.as_nanos() as i64 / 100 + EPOCH_DIFF_TICKS
}

pub fn read(manager: &Manager, cache: &mut ThumbCache) -> windows::core::Result<Option<MediaInfo>> {
    // A null session (nothing playing anywhere) comes back as Err.
    let Ok(session) = manager.GetCurrentSession() else {
        return Ok(None);
    };
    let props = session.TryGetMediaPropertiesAsync()?.get()?;
    let title = props.Title()?.to_string();
    if title.is_empty() {
        return Ok(None);
    }
    let artist = props.Artist()?.to_string();
    let app_id = session.SourceAppUserModelId()?.to_string();
    let playing = session.GetPlaybackInfo()?.PlaybackStatus()? == Status::Playing;

    // Apps only update the timeline on seek/play/pause, so extrapolate from the
    // last update while playing.
    let tl = session.GetTimelineProperties()?;
    let start = tl.StartTime()?.Duration;
    let duration = (tl.EndTime()?.Duration - start).max(0);
    let mut pos = tl.Position()?.Duration - start;
    if playing {
        pos += (now_ticks() - tl.LastUpdatedTime()?.UniversalTime).max(0);
    }
    let pos = if duration > 0 { pos.clamp(0, duration) } else { pos.max(0) };

    let key = format!("{app_id}|{title}|{artist}");
    if cache.key != key {
        cache.key = key;
        cache.data = None;
        cache.rereads = REREADS_AFTER_CHANGE;
    }
    if cache.data.is_none() || cache.rereads > 0 {
        cache.rereads = cache.rereads.saturating_sub(1);
        if let Some(art) = read_thumbnail(&props).ok().flatten() {
            cache.data = Some(art);
        }
    }

    Ok(Some(MediaInfo {
        title,
        artist,
        album: props.AlbumTitle()?.to_string(),
        app_id,
        playing,
        position_ms: (pos / 10_000) as u64,
        duration_ms: (duration / 10_000) as u64,
        thumbnail: cache.data.clone(),
    }))
}

fn read_thumbnail(props: &MediaProperties) -> windows::core::Result<Option<String>> {
    match props.Thumbnail() {
        Ok(thumb) => image_data_url(&thumb),
        Err(_) => Ok(None),
    }
}

/// Reads a WinRT image stream (album art, app logo) into a data URL.
pub fn image_data_url(image: &IRandomAccessStreamReference) -> windows::core::Result<Option<String>> {
    let stream = image.OpenReadAsync()?.get()?;
    let size = stream.Size()? as u32;
    if size == 0 {
        return Ok(None);
    }
    let reader = DataReader::CreateDataReader(&stream.GetInputStreamAt(0)?)?;
    reader.LoadAsync(size)?.get()?;
    let mut buf = vec![0u8; size as usize];
    reader.ReadBytes(&mut buf)?;
    let mime = stream.ContentType().map(|s| s.to_string()).unwrap_or_default();
    let mime = if mime.is_empty() { "image/png".to_string() } else { mime };
    let b64 = base64::engine::general_purpose::STANDARD.encode(&buf);
    Ok(Some(format!("data:{mime};base64,{b64}")))
}

pub enum Action {
    PlayPause,
    Next,
    Previous,
}

pub fn control(action: Action) -> Result<(), String> {
    let run = || -> windows::core::Result<bool> {
        let session = manager()?.GetCurrentSession()?;
        match action {
            Action::PlayPause => session.TryTogglePlayPauseAsync()?.get(),
            Action::Next => session.TrySkipNextAsync()?.get(),
            Action::Previous => session.TrySkipPreviousAsync()?.get(),
        }
    };
    run().map(|_| ()).map_err(|e| e.message().to_string())
}
