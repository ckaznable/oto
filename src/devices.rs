use std::{collections::HashSet, fs, path::Path};

use alsa::{Ctl, Direction, card::Iter, ctl::DeviceIter};

#[derive(Debug, Clone)]
pub struct AudioPCM {
    pub index: i32,
    pub name: Option<String>,
    pub long_name: Option<String>,
    pub caps: CardCapabilities,
    pub devices: Vec<AudioDevice>,
}

impl AudioPCM {
    pub fn get_default(&self) -> (i32, i32) {
        match self.devices.first() {
            Some(d) => (self.index, d.index),
            _ => (self.index, 0),
        }
    }
}

#[derive(Debug, Clone)]
pub struct AudioDevice {
    pub index: i32,
    pub name: Option<String>,
}

#[derive(Debug, Clone)]
pub struct CardCapabilities {
    pub formats: Vec<String>,
    pub is_usb_dac: bool,
    pub rates: Vec<u32>,
    pub channels: Vec<u32>,
    pub dsd: bool,
    pub dop: bool,
}

impl Default for CardCapabilities {
    fn default() -> Self {
        Self {
            is_usb_dac: false,
            formats: vec!["S16_LE".to_string(), "S32_LE".to_string()],
            rates: vec![44100, 48000],
            channels: vec![2],
            dsd: false,
            dop: false,
        }
    }
}

pub fn get_default_device(pcm: &[AudioPCM]) -> Option<(i32, i32)> {
    pcm.iter()
        .find(|p| p.caps.is_usb_dac)
        .map(|p| p.get_default())
        .or_else(|| pcm.first().map(|p| p.get_default()))
}

pub fn list_devices() -> Vec<AudioPCM> {
    let mut pcm: Vec<AudioPCM> = Vec::new();

    let iter = Iter::new();
    for card in iter.flatten() {
        let index = card.get_index();
        let caps = get_stream0_caps(index as u32).unwrap_or_default();
        let Ok(ctl) = Ctl::from_card(&card, false) else {
            continue;
        };

        let devices: Vec<AudioDevice> = DeviceIter::new(&ctl)
            .filter_map(|index| {
                let info = ctl.pcm_info(index as u32, 0, Direction::Playback).ok()?;
                Some(AudioDevice {
                    index,
                    name: info.get_name().map(|s| s.to_owned()).ok(),
                })
            })
            .collect();

        pcm.push(AudioPCM {
            index,
            caps,
            name: card.get_name().ok(),
            long_name: card.get_longname().ok(),
            devices,
        });
    }
    pcm
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
        is_usb_dac: true,
        dsd: supports_dsd,
        dop: supports_dop,
    })
}
