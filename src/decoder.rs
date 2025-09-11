use std::{
    collections::VecDeque, fmt::Display, path::PathBuf
};

use anyhow::{anyhow, Result};

use symphonia::core::{
    audio::{
        AudioBuffer,
        AudioBufferRef,
        SampleBuffer
    },
    codecs::{
        DecoderOptions,
        CODEC_TYPE_NULL
    },
    errors::Error,
    formats::{
        FormatOptions,
        FormatReader
    },
    io::MediaSourceStream,
    meta::MetadataOptions,
    probe::Hint
};

use crate::media::MediaSpec;

#[allow(clippy::upper_case_acronyms)]
#[derive(Debug, Clone)]
pub enum DecoderError {
    EOF,
    Ignored,
    Raw(String)
}

impl std::error::Error for DecoderError {}

impl Display for DecoderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DecoderError::EOF => write!(f, "eof"),
            DecoderError::Ignored => write!(f, "Ignored"),
            DecoderError::Raw(s) => write!(f, "{s}"),
        }
    }
}

pub trait Decoder {
    fn decode(&mut self, buf: &mut VecDeque<i32>) -> Result<(), DecoderError>;
    fn spec(&self) -> Option<MediaSpec>;
}

#[derive(Default)]
pub struct DecoderManager {
    decoder: Option<Box<dyn Decoder>>,
}

impl DecoderManager {
    pub fn open(&mut self, p: PathBuf) -> Result<()> {
        let file = std::fs::File::open(p)?;
        self.decoder.replace(Box::new(PcmDecoder::new(file)?));
        Ok(())
    }
}

impl Decoder for DecoderManager {
    #[inline]
    fn spec(&self) -> Option<MediaSpec> {
        self.decoder.as_ref().and_then(|d| d.spec())
    }

    fn decode(&mut self, buf: &mut VecDeque<i32>) -> Result<(), DecoderError> {
        if let Some(decoder) = self.decoder.as_mut() {
            decoder.decode(buf)?;
        }

        Ok(())
    }
}

pub struct PcmDecoder {
    format: Box<dyn FormatReader>,
    track_id: u32,
    decoder: Box<dyn symphonia::core::codecs::Decoder>,
}

impl PcmDecoder {
    fn new(src: std::fs::File) -> Result<Self> {
        // Create the media source stream.
        let mss = MediaSourceStream::new(Box::new(src), Default::default());

        // Create a probe hint using the file's extension. [Optional]
        let hint = Hint::new();

        // Use the default options for metadata and format readers.
        let meta_opts = MetadataOptions::default();
        let fmt_opts = FormatOptions::default();

        // Probe the media source.
        let probed = symphonia::default::get_probe()
            .format(&hint, mss, &fmt_opts, &meta_opts)
            .map_err(|_| anyhow!("unsupported format"))?;

        // Get the instantiated format reader.
        let format = probed.format;

        let track = format
            .tracks()
            .iter()
            .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
            .ok_or(anyhow!("no supported audio tracks"))?;

        // Use the default options for the decoder.
        let dec_opts = DecoderOptions::default();

        // Create a decoder for the track.
        let decoder = symphonia::default::get_codecs()
            .make(&track.codec_params, &dec_opts)?;

        // Store the track identifier and decoder
        let track_id = track.id;
        let decoder = decoder;

        Ok(Self {
            format,
            track_id,
            decoder,
        })
    }
}

impl Decoder for PcmDecoder {
    fn spec(&self) -> Option<MediaSpec> {
        let params = self.decoder.codec_params();
        Some(MediaSpec {
            sample_rate: params.sample_rate?,
            channel: params.channels.map(|c| c.count() as u32)?,
            mode: crate::media::OutputMode::PCM,
        })
    }

    fn decode(&mut self, buf: &mut VecDeque<i32>) -> Result<(), DecoderError> {
        // Get the next packet from the media format.
        let packet = match self.format.next_packet() {
            Ok(packet) => packet,
            Err(Error::ResetRequired) => {
                // The track list has been changed. Re-examine it and create a new set of decoders,
                // then restart the decode loop. This is an advanced feature and it is not
                // unreasonable to consider this "the end." As of v0.5.0, the only usage of this is
                // for chained OGG physical streams.
                return Err(DecoderError::Ignored);
            }
            Err(Error::IoError(err)) if err.kind() == std::io::ErrorKind::UnexpectedEof => {
                return Err(DecoderError::EOF)
            },
            Err(err) => {
                // A unrecoverable error occurred, halt decoding.
                return Err(DecoderError::Raw(err.to_string()))
            }
        };

        // Consume any new metadata that has been read since the last packet.
        while !self.format.metadata().is_latest() {
            // Pop the old head of the metadata queue.
            self.format.metadata().pop();
        }

        // If the packet does not belong to the selected track, skip over it.
        if packet.track_id() != self.track_id {
            return Err(DecoderError::Ignored);
        }

        match self.decoder.decode(&packet) {
            Ok(_decoded) => {
                // Consume the decoded audio samples (see below).
                let duration = _decoded.capacity() as u64;
                let spec = _decoded.spec().to_owned();
                let mut sb: SampleBuffer<i32> = SampleBuffer::new(duration, spec);
                match _decoded {
                    AudioBufferRef::S32(b) => {
                        sb.copy_interleaved_typed(b.as_ref());
                    }
                    _ => {
                        let mut buf: AudioBuffer<i32> = AudioBuffer::new(duration, spec);
                        _decoded.convert(&mut buf);
                        sb.copy_interleaved_typed(&buf);
                    }
                }

                let data = sb.samples();
                buf.extend(data);
                Ok(())
            }
            Err(Error::IoError(_)) => {
                // The packet failed to decode due to an IO error, skip the packet.
                Err(DecoderError::Ignored)
            }
            Err(Error::DecodeError(_)) => {
                // The packet failed to decode due to invalid data, skip the packet.
                Err(DecoderError::Ignored)
            }
            Err(err) => {
                // An unrecoverable error occurred, halt decoding.
                Err(DecoderError::Raw(err.to_string()))
            }
        }
    }
}

