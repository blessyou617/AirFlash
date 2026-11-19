//! Fixed-input protocol comparison, not a sender-side LED animation.
//! Wire shapes follow music-assistant/airplay-cli ap2_mrp.c at
//! 0a58c239d5fb3cf8927b9707a71c6fe897266b6a. Reference source:
//! Copyright (C) 2024-2026 Music Assistant Contributors, Apache-2.0.
//! Added for this experimental build on 2026-11-20; GPL-3.0-or-later.
use anyhow::Result;
use plist::{Dictionary, Value};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::time::SystemTime;
use uuid::Uuid;

pub const TITLE: &str = "AirFlash Sound Glow Protocol Test";
pub const ARTIST: &str = "AirFlash";
pub const ALBUM: &str = "Fixed Input Comparison v1";
pub const ART: &[u8] = include_bytes!("../assets/sound-glow/protocol.jpg");
pub const TEST_SECONDS: u64 = 64;

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Experiment {
    pub media_remote: bool,
}

#[derive(Clone, Debug)]
pub struct Identity {
    pub session_uuid: String,
    pub group_uuid: Option<String>,
    pub device_uuid: String,
    pub dacp_id: String,
}
impl Default for Identity {
    fn default() -> Self {
        Self {
            session_uuid: Uuid::new_v4().to_string(),
            group_uuid: None,
            device_uuid: Uuid::new_v4().to_string(),
            dacp_id: format!("{:016X}", rand::random::<u64>()),
        }
    }
}

fn dict(entries: impl IntoIterator<Item = (&'static str, Value)>) -> Value {
    Value::Dictionary(entries.into_iter().map(|(k, v)| (k.to_owned(), v)).collect::<Dictionary>())
}
fn int(value: u64) -> Value { Value::Integer(value.into()) }
fn string(value: &str) -> Value { Value::String(value.into()) }
fn serialize(value: Value) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    value.to_writer_binary(&mut out)?;
    Ok(out)
}
fn command(kind: &str, params: Value) -> Result<Vec<u8>> {
    serialize(dict([("type", string(kind)), ("params", params)]))
}
fn varint(out: &mut Vec<u8>, mut value: u64) {
    while value >= 128 { out.push((value as u8 & 127) | 128); value >>= 7; }
    out.push(value as u8);
}
fn number_field(out: &mut Vec<u8>, field: u64, value: u64) {
    varint(out, field << 3); varint(out, value);
}
fn bytes_field(out: &mut Vec<u8>, field: u64, value: &[u8]) {
    varint(out, (field << 3) | 2); varint(out, value.len() as u64); out.extend_from_slice(value);
}
pub fn device_info(id: &Identity) -> Result<Vec<u8>> {
    let mut inner = Vec::new();
    for (field, value) in [
        (1, id.device_uuid.as_str()), (2, "AirFlash Protocol Test"), (3, "iPhone"),
        (4, "21F90"), (5, "com.apple.Music"), (12, "com.apple.Music"),
        (19, id.dacp_id.as_str()), (31, "com.apple.podcasts"), (39, "iPhone17,1"),
        (41, id.session_uuid.as_str()), (43, "com.apple.iBooks"),
    ] { bytes_field(&mut inner, field, value.as_bytes()); }
    if let Some(group) = &id.group_uuid { bytes_field(&mut inner, 42, group.as_bytes()); }
    for (field, value) in [(7,1),(8,139),(9,1),(10,1),(13,1),(14,1),(15,1),(17,2),(21,1),(22,1)] {
        number_field(&mut inner, field, value);
    }
    let mut envelope = Vec::new();
    number_field(&mut envelope, 1, 15);
    bytes_field(&mut envelope, 20, &inner);
    bytes_field(&mut envelope, 85, Uuid::new_v4().to_string().as_bytes());
    let mut framed = Vec::new();
    varint(&mut framed, envelope.len() as u64);
    framed.extend(envelope);
    serialize(dict([("params", dict([("data", Value::Data(framed))]))]))
}
pub fn artwork_hash() -> String { format!("{:x}", Sha256::digest(ART)) }

pub fn now_playing(elapsed: f64, now: SystemTime) -> Result<Vec<u8>> {
    let info = dict([
        ("kMRMediaRemoteNowPlayingInfoTitle", string(TITLE)),
        ("kMRMediaRemoteNowPlayingInfoArtist", string(ARTIST)),
        ("kMRMediaRemoteNowPlayingInfoAlbum", string(ALBUM)),
        ("kMRMediaRemoteNowPlayingInfoElapsedTime", Value::Real(elapsed)),
        ("kMRMediaRemoteNowPlayingInfoPlaybackRate", Value::Real(1.0)),
        ("kMRMediaRemoteNowPlayingInfoDefaultPlaybackRate", Value::Real(1.0)),
        ("kMRMediaRemoteNowPlayingInfoTimestamp", Value::Date(now.into())),
        ("kMRMediaRemoteNowPlayingInfoMediaType", string("MRMediaRemoteMediaTypeMusic")),
        ("kMRMediaRemoteNowPlayingInfoUniqueIdentifier", int(0x4146474c4f573031)),
        ("kMRMediaRemoteNowPlayingInfoArtworkIdentifier", string(&artwork_hash())),
        ("kMRMediaRemoteNowPlayingInfoArtworkMIMEType", string("image/jpeg")),
        ("kMRMediaRemoteNowPlayingInfoArtworkData", Value::Data(ART.to_vec())),
    ]);
    command("updateMRNowPlayingInfo", dict([
        ("type", string("npi-text")), ("mergePolicy", string("replace")), ("params", info),
    ]))
}
pub fn supported_commands() -> Result<Vec<u8>> {
    // Match the reference advertisement for the experiment. This is not a
    // complete remote-control implementation; use desktop controls in the test.
    let mut commands = Vec::new();
    for code in [26u64,25,24,18,17,10,11,8,9,5,4,3,2,1,0] {
        let options = match code {
            26 => Some(dict([("kMRMediaRemoteCommandInfoShuffleMode", int(1))])),
            25 => Some(dict([("kMRMediaRemoteCommandInfoRepeatMode", int(1))])),
            24 => Some(dict([
                ("kMRMediaRemoteCommandInfoCanBeControlledByScrubbingKey", Value::Boolean(false)),
                ("kMRMediaRemoteCommandInfoSupportsReferencePosition", Value::Boolean(false)),
            ])),
            17 | 18 => Some(dict([("kMRMediaRemoteCommandInfoPreferredIntervalsKey", Value::Array(vec![]))])),
            _ => None,
        };
        let mut item = dict([
            ("kCommandInfoCommandKey", int(code)),
            ("kCommandInfoEnabledKey", Value::Boolean(code != 17 && code != 18)),
        ]);
        if let Some(options) = options { item.as_dictionary_mut().unwrap().insert("kCommandInfoOptionsKey".into(), options); }
        commands.push(Value::Data(serialize(item)?));
    }
    command("updateMRSupportedCommands", dict([("mrSupportedCommandsFromSender", Value::Array(commands))]))
}
pub fn playback_state() -> Result<Vec<u8>> {
    command("updateMRPlaybackState", dict([("mrPlaybackState", int(1))]))
}
pub fn now_playing_client() -> Result<Vec<u8>> {
    let mut data = Vec::new();
    number_field(&mut data, 1, std::process::id() as u64);
    bytes_field(&mut data, 2, b"com.apple.Music");
    bytes_field(&mut data, 7, b"AirFlash Protocol Test");
    command("updateMRNowPlayingClient", dict([("mrNowPlayingClient", Value::Data(data))]))
}
pub fn metadata() -> Vec<u8> {
    fn tag(out: &mut Vec<u8>, key: &[u8;4], data: &[u8]) {
        out.extend_from_slice(key); out.extend_from_slice(&(data.len() as u32).to_be_bytes()); out.extend_from_slice(data);
    }
    let mut fields = Vec::new();
    for (key, text) in [(b"minm",TITLE),(b"asar",ARTIST),(b"asal",ALBUM)] { tag(&mut fields,key,text.as_bytes()); }
    let mut out = Vec::new(); tag(&mut out,b"mlit",&fields); out
}

/// Deterministic, identical stereo channels; generated once outside the audio
/// loop. The waveform never drives metadata or cover updates.
pub fn test_samples(rate: u32) -> Vec<i16> {
    let mut out = Vec::with_capacity(rate as usize * TEST_SECONDS as usize * 2);
    for frame in 0..rate as u64 * TEST_SECONDS {
        let t = frame as f64 / rate as f64;
        let beat = (t - 8.0).rem_euclid(0.5);
        let kick = (std::f64::consts::TAU * (55.0 * beat + 6.0 * (1.0 - (-35.0 * beat).exp()))).sin() * (-18.0 * beat).exp();
        let envelope = if !(8.0..56.0).contains(&t) { 0.0 } else { 1.0 };
        let value = if t < 24.0 {
            kick * if ((t - 8.0) / 0.5) as i32 % 4 == 0 { 0.35 } else { 0.17 }
        } else if t < 40.0 {
            let notes = [220.0, 261.6256, 329.6276, 293.6648];
            let note = notes[((t - 24.0) / 2.0) as usize % notes.len()];
            kick * 0.24 + (std::f64::consts::TAU * note * t).sin() * 0.06 * (-3.0 * beat).exp()
        } else {
            // A steady carrier distinguishes beat-locked movement from a
            // generic continuously moving animation. Fade at both boundaries.
            let fade = ((t - 40.0) * 20.0).clamp(0.0,1.0) * ((56.0-t)*20.0).clamp(0.0,1.0);
            (std::f64::consts::TAU * 220.0 * t).sin() * 0.10 * fade
        } * envelope;
        let sample = (value * 32767.0).round() as i16;
        out.extend([sample, sample]);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    fn decode(bytes: &[u8]) -> Value { Value::from_reader(std::io::Cursor::new(bytes)).unwrap() }
    fn get<'a>(value: &'a Value, key: &str) -> &'a Value { &value.as_dictionary().unwrap()[key] }
    #[test]
    fn native_state_has_typed_time_music_and_identical_artwork() {
        let now = SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1_800_000_000);
        let body = decode(&now_playing(3.0, now).unwrap());
        assert_eq!(get(&body,"type").as_string(),Some("updateMRNowPlayingInfo"));
        let p = get(&body,"params");
        assert_eq!(get(p,"mergePolicy").as_string(),Some("replace"));
        let info = get(p,"params");
        assert_eq!(get(info,"kMRMediaRemoteNowPlayingInfoArtworkData").as_data(),Some(ART));
        assert_eq!(get(info,"kMRMediaRemoteNowPlayingInfoElapsedTime").as_real(),Some(3.0));
        assert!(get(info,"kMRMediaRemoteNowPlayingInfoTimestamp").as_date().is_some());
        assert_eq!(get(info,"kMRMediaRemoteNowPlayingInfoMediaType").as_string(),Some("MRMediaRemoteMediaTypeMusic"));
        let commands = decode(&supported_commands().unwrap());
        let list = get(get(&commands,"params"),"mrSupportedCommandsFromSender").as_array().unwrap();
        assert_eq!(list.len(),15);
        for entry in list { assert!(decode(entry.as_data().unwrap()).as_dictionary().is_some()); }
        assert_eq!(get(get(&decode(&playback_state().unwrap()),"params"),"mrPlaybackState").as_unsigned_integer(),Some(1));
    }
    #[test]
    fn protobuf_device_frame_matches_setup_routing_and_client_is_bare() {
        let id = Identity { session_uuid:"routing-context".into(), group_uuid:Some("airplay-group".into()), ..Default::default() };
        let body = decode(&device_info(&id).unwrap());
        let bytes = get(get(&body,"params"),"data").as_data().unwrap();
        let (mut len,mut shift,mut at)=(0usize,0,0);
        loop { let b=bytes[at];at+=1;len|=((b&127)as usize)<<shift; if b<128 {break;} shift+=7; }
        assert_eq!(len,bytes.len()-at);
        assert_eq!(&bytes[at..at+2], &[0x08,15]);
        // field 41 (routingContextID), field 42 (airPlayGroupID).
        assert!(bytes.windows(18).any(|v| v == b"\xca\x02\x0frouting-context"));
        assert!(bytes.windows(16).any(|v| v == b"\xd2\x02\x0dairplay-group"));
        let client=decode(&now_playing_client().unwrap());
        let data=get(get(&client,"params"),"mrNowPlayingClient").as_data().unwrap();
        assert_eq!(data[0],0x08);
        assert!(data.windows(15).any(|v|v==b"com.apple.Music"));
    }
    #[test]
    fn fixture_contains_silence_beats_steady_sound_and_equal_channels() {
        let s=test_samples(8000);
        assert_eq!(s.len(),64*8000*2);
        assert!(s[..8*8000*2].iter().all(|s|*s==0));
        assert!(s[56*8000*2..].iter().all(|s|*s==0));
        assert!(s.chunks_exact(2).all(|v|v[0]==v[1]));
        assert!(s.iter().all(|v|v.unsigned_abs()<=11500));
        assert!(s[8*8000*2..24*8000*2].iter().any(|s|s.abs()>4000));
        assert!(s[42*8000*2..54*8000*2].iter().any(|s|s.abs()>3000));
    }
}
