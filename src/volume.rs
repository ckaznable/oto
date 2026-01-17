use alsa::{
    Mixer,
    mixer::{Selem, SelemChannelId, SelemId},
};
use anyhow::{Result, anyhow};

pub struct VolumeController {
    mixer_name: String,
}

impl VolumeController {
    pub fn new(card: &str) -> Self {
        let mut mixer_name = card.to_string();
        if mixer_name.starts_with("hw")
            && mixer_name.contains(",")
            && let Some(name) = mixer_name.split(",").next()
        {
            mixer_name = name.to_owned();
        }

        log::debug!("card name to config volume controller: {mixer_name}");

        Self { mixer_name }
    }

    pub fn set_device(&mut self, card: &str) {
        self.mixer_name = card.to_string();
    }

    pub fn get_selem<'a>(&self, mixer: &'a Mixer) -> Result<Selem<'a>> {
        let sid_master = SelemId::new("Master", 0);
        let sid_pcm = SelemId::new("PCM", 0);

        let selem = mixer
            .find_selem(&sid_master)
            .or_else(|| mixer.find_selem(&sid_pcm))
            .ok_or(anyhow!("can't not found selem"))?;

        Ok(selem)
    }

    pub fn set_volume(&self, percent: i64) -> Result<()> {
        let mixer = Mixer::new(&self.mixer_name, false)?;
        let selem = self.get_selem(&mixer)?;
        let (min, max) = selem.get_playback_volume_range();

        let range = max - min;
        let target = min + (range * percent / 100);

        selem.set_playback_volume_all(target)?;

        Ok(())
    }

    pub fn get_volume(&self) -> Result<i64> {
        let mixer = Mixer::new(&self.mixer_name, false)?;
        let selem = self.get_selem(&mixer)?;

        let (min, max) = selem.get_playback_volume_range();

        let current = selem.get_playback_volume(SelemChannelId::FrontLeft)?;

        let range = max - min;
        if range == 0 {
            return Ok(0);
        }

        let percent = (current - min) * 100 / range;
        Ok(percent)
    }

    pub fn set_mute(&self, mute: bool) -> Result<()> {
        let mixer = Mixer::new(&self.mixer_name, false)?;
        let selem = self.get_selem(&mixer)?;

        let val = if mute { 0 } else { 1 };
        selem.set_playback_switch_all(val)?;
        Ok(())
    }
}
