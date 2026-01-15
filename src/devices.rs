use std::{collections::HashSet, fs, path::Path};

use alsa::card::Iter;

#[derive(Debug)]
pub struct AudioDevice {
    pub index: i32,
    pub name: Option<String>,
    pub long_name: Option<String>,
    pub caps: CardCapabilities,
}

#[derive(Debug, Default, Clone)]
pub struct CardCapabilities {
    pub formats: Vec<String>,
    pub rates: Vec<u32>,
    pub channels: Vec<u32>,
    pub dsd: bool,
    pub dop: bool,
}

pub fn list_devices() -> Vec<AudioDevice> {
    let mut devices = Vec::new();

    let iter = Iter::new();
    for card in iter.flatten() {
        let index = card.get_index();
        if let Some(caps) = get_stream0_caps(index as u32) {
            devices.push(AudioDevice {
                index,
                caps,
                name: card.get_name().ok(),
                long_name: card.get_longname().ok(),
            });
        }
    }
    devices
}

pub fn get_stream0_caps(card_num: u32) -> Option<CardCapabilities> {
    let path_str = format!("/proc/asound/card{}/stream0", card_num);
    let path = Path::new(&path_str);

    if !path.exists() {
        return None;
    }

    let content = fs::read_to_string(path).ok()?;

    let mut formats_set = HashSet::new();
    let mut rates_set = HashSet::new();
    let mut channels_set = HashSet::new();
    let mut supports_dsd = false;

    // Dop detection
    let mut has_176k = false;
    // S24 or S32
    let mut has_high_bit_depth = false;

    let mut is_playback_section = false;

    for line in content.lines() {
        let line = line.trim();

        if line.starts_with("Playback:") {
            is_playback_section = true;
            continue;
        } else if line.starts_with("Capture:") {
            is_playback_section = false;
            continue;
        }

        if !is_playback_section {
            continue;
        }

        // parse Format
        if let Some(val) = line.strip_prefix("Format: ") {
            let fmt = val.trim().to_string();

            if fmt.contains("DSD") {
                supports_dsd = true;
            }
            if fmt.contains("S24") || fmt.contains("S32") {
                has_high_bit_depth = true;
            }

            formats_set.insert(fmt);
        }

        // parse Rates
        if let Some(val) = line.strip_prefix("Rates: ") {
            let parts: Vec<&str> = val.split(',').collect();
            for p in parts {
                if let Ok(rate) = p.trim().parse::<u32>() {
                    rates_set.insert(rate);
                    if rate == 176400 {
                        has_176k = true;
                    }
                }
            }
        }

        // parse Channels
        if let Some(val) = line.strip_prefix("Channels: ")
            && let Ok(ch) = val.trim().parse::<u32>()
        {
            channels_set.insert(ch);
        }
    }

    let mut rates: Vec<u32> = rates_set.into_iter().collect();
    rates.sort();

    let mut formats: Vec<String> = formats_set.into_iter().collect();
    formats.sort();

    let mut channels: Vec<u32> = channels_set.into_iter().collect();
    channels.sort();

    let supports_dop = has_176k && has_high_bit_depth;

    Some(CardCapabilities {
        formats,
        rates,
        channels,
        dsd: supports_dsd,
        dop: supports_dop,
    })
}
