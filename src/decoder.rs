use std::{
    collections::VecDeque,
    io::{Cursor, Read, Seek, SeekFrom},
    path::PathBuf,
};

use anyhow::{Result, anyhow};

use bytemuck::cast_slice;
use id3::Tag;
use symphonia::core::{
    audio::{AudioBuffer, AudioBufferRef, SampleBuffer},
    codecs::{CODEC_TYPE_NULL, DecoderOptions},
    errors::Error,
    formats::{FormatOptions, FormatReader},
    io::{MediaSource, MediaSourceStream},
    meta::MetadataOptions,
    probe::Hint,
};

use crate::media::MediaSpec;

#[allow(clippy::upper_case_acronyms)]
#[derive(Debug, Clone, thiserror::Error)]
pub enum DecoderError {
    #[error("EOF")]
    EOF,
    #[error("Ignored")]
    Ignored,
    #[error("{0}")]
    Raw(String),
}

pub trait Decoder {
    fn decode(&mut self, buf: &mut VecDeque<i32>) -> Result<(), DecoderError>;
    fn spec(&self) -> Option<MediaSpec>;
}

#[derive(Default)]
pub struct MixDecoder {
    pub file_path: Option<PathBuf>,
    decoder: Option<Box<dyn Decoder>>,
}

impl MixDecoder {
    pub fn open(&mut self, p: PathBuf) -> Result<()> {
        self.file_path = Some(p.clone());
        let mut file = std::fs::File::open(&p)?;
        let is_dsd_file = Self::is_dsd_file(&mut file)?;
        file.seek(SeekFrom::Start(0))?;

        let decoder: Box<dyn Decoder> = if is_dsd_file {
            Box::new(DsdDecoder::new(file)?)
        } else {
            Box::new(PcmDecoder::new(
                file,
                p.extension().and_then(|e| e.to_str()),
            )?)
        };

        self.decoder.replace(decoder);
        Ok(())
    }

    fn is_dsd_file(r: &mut std::fs::File) -> Result<bool> {
        let mut buf = [0u8; 4];
        r.read_exact(&mut buf)?;
        Ok(&buf == b"DSD ")
    }
}

impl Decoder for MixDecoder {
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
    fn new<S>(src: S, extname: Option<&str>) -> Result<Self>
    where
        S: MediaSource + 'static,
    {
        // Create the media source stream.
        let mss = MediaSourceStream::new(Box::new(src), Default::default());

        // Create a probe hint using the file's extension. [Optional]
        let mut hint = Hint::new();
        if let Some(ext) = extname {
            hint.with_extension(ext);
        }

        // Use the default options for metadata and format readers.
        let meta_opts = MetadataOptions::default();
        let fmt_opts = FormatOptions::default();

        // Probe the media source.
        let probed = symphonia::default::get_probe()
            .format(&hint, mss, &fmt_opts, &meta_opts)
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;

        // Get the instantiated format reader.
        let format = probed.format;

        let track = format
            .tracks()
            .iter()
            .find(|t| t.codec_params.codec != CODEC_TYPE_NULL && t.codec_params.sample_rate.is_some())
            .ok_or(anyhow!("no supported audio tracks"))?;

        // Use the default options for the decoder.
        let dec_opts = DecoderOptions::default();
        // Create a decoder for the track.
        let decoder = symphonia::default::get_codecs().make(&track.codec_params, &dec_opts)?;

        // Store the track identifier and decoder
        let track_id = track.id;

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
        let sample_rate = params.sample_rate?;
        Some(MediaSpec {
            sample_rate,
            duration: params.n_frames.map(|s| s / sample_rate as u64),
            channels: params.channels.map(|c| c.count() as u32)?,
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
                return Err(DecoderError::EOF);
            }
            Err(err) => {
                // A unrecoverable error occurred, halt decoding.
                return Err(DecoderError::Raw(err.to_string()));
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

#[derive(Debug)]
pub struct DsfMetadata {
    pub dsd_chunk_size: u64,
    pub fmt_chunk_size: u64,
    pub data_chunk_size: u64,
    pub file_size: u64,
    pub pointer_to_metadata_chunk: u64,
    pub format_version: u32,
    pub format_id: u32,
    pub channel_type: u32,
    pub channel_num: u32,
    pub sample_freq: u32,
    pub bps: u32,
    pub sample_count: u64,
    pub channel_block_size: u32,
    pub tag: Option<Tag>,
}

pub struct DsfReader<'a, R> {
    reader: &'a mut R,
    u32_buf: [u8; 4],
    u64_buf: [u8; 8],
}

impl<'a, R: Read + Seek> DsfReader<'a, R> {
    pub fn new(reader: &'a mut R) -> Self {
        Self {
            reader,
            u32_buf: [0u8; 4],
            u64_buf: [0u8; 8],
        }
    }

    pub fn parse(mut self) -> Result<DsfMetadata> {
        self.reader.seek(SeekFrom::Start(0))?;

        // 'DSD '
        if &self.read_u32()?.to_le_bytes() != b"DSD " {
            return Err(anyhow!("not dsf file"));
        }
        // size of dsd chunk
        let dsd_chunk_size = self.read_u64()?;
        // totol file size
        let file_size = self.read_u64()?;
        // pointer to metadata chunk
        let pointer_to_metadata_chunk = self.read_u64()?;
        // 'fmt '
        if &self.read_u32()?.to_le_bytes() != b"fmt " {
            return Err(anyhow!("not dsf file"));
        }
        // size of fmt chunk
        let fmt_chunk_size = self.read_u64()?;
        // format version
        let format_version = self.read_u32()?;
        // format id
        let format_id = self.read_u32()?;
        // channel type
        let channel_type = self.read_u32()?;
        // channel num
        let channel_num = self.read_u32()?;
        // sampleling frequency
        let sample_freq = self.read_u32()?;
        // bit per sample
        let bps = self.read_u32()?;
        // sample count
        let sample_count = self.read_u64()?;
        // block size per channel
        let channel_block_size = self.read_u32()?;
        // reserved
        self.read_u32()?;
        // 'data'
        if &self.read_u32()?.to_le_bytes() != b"data" {
            return Err(anyhow!("not dsf file"));
        }
        // size of data chunk
        let data_chunk_size = self.read_u64()?;

        let dsd_size = dsd_chunk_size + fmt_chunk_size + data_chunk_size;
        if dsd_size > file_size {
            return Err(anyhow!("dsd file parser error"));
        }

        let metadata_size = file_size - dsd_size;
        self.reader
            .seek(SeekFrom::Start(pointer_to_metadata_chunk))?;

        let mut metadata = vec![0u8; metadata_size as usize];
        self.reader.read_exact(&mut metadata)?;

        let tag = id3::v1v2::read_from(Cursor::new(metadata)).ok();

        Ok(DsfMetadata {
            dsd_chunk_size,
            fmt_chunk_size,
            data_chunk_size,
            file_size,
            pointer_to_metadata_chunk,
            format_version,
            format_id,
            channel_type,
            channel_num,
            sample_freq,
            bps,
            sample_count,
            channel_block_size,
            tag,
        })
    }

    fn read_u32(&mut self) -> Result<u32> {
        self.reader.read_exact(&mut self.u32_buf)?;
        Ok(u32::from_le_bytes(self.u32_buf))
    }

    fn read_u64(&mut self) -> Result<u64> {
        self.reader.read_exact(&mut self.u64_buf)?;
        Ok(u64::from_le_bytes(self.u64_buf))
    }
}

pub struct DsdDecoder<R> {
    spec: MediaSpec,
    metadata: DsfMetadata,
    reader: R,
    read: usize,
    raw_block_buf: Vec<u8>,
}

impl<R: Read + Seek> DsdDecoder<R> {
    pub fn new(mut reader: R) -> Result<Self> {
        let dsf_reader = DsfReader::new(&mut reader);
        let mut metadata = dsf_reader.parse()?;

        // pop tag data
        metadata.tag.take();

        let spec = MediaSpec {
            sample_rate: metadata.sample_freq,
            duration: Some(metadata.sample_count / metadata.sample_freq as u64),
            channels: metadata.channel_num,
            mode: crate::media::OutputMode::DSD,
        };

        let raw_buffer_size = (metadata.channel_block_size * spec.channels) as usize;

        // reset reader to data position
        reader.seek(SeekFrom::Start(
            metadata.dsd_chunk_size + metadata.fmt_chunk_size + 12,
        ))?;

        Ok(Self {
            spec,
            metadata,
            reader,
            read: 0,
            raw_block_buf: vec![0u8; raw_buffer_size],
        })
    }

    pub fn reset(&mut self) -> anyhow::Result<()> {
        self.reader.seek(SeekFrom::Start(
            self.metadata.dsd_chunk_size + self.metadata.fmt_chunk_size + 12,
        ))?;
        Ok(())
    }
}

impl<R: Read + Seek> Decoder for DsdDecoder<R> {
    fn decode(&mut self, buf: &mut VecDeque<i32>) -> Result<(), DecoderError> {
        if self.read >= self.metadata.data_chunk_size as usize {
            return Err(DecoderError::EOF);
        }

        // channels block size (bytes)
        let block_bytes = self.metadata.channel_block_size as usize;
        let channels = self.spec.channels as usize;
        let total_bytes_to_read = block_bytes * channels;

        let mut read_offset = 0;
        while read_offset < total_bytes_to_read {
            match self.reader.read(&mut self.raw_block_buf[read_offset..]) {
                Ok(0) => return Err(DecoderError::EOF),
                Ok(n) => read_offset += n,
                Err(_) => return Err(DecoderError::Raw("read error".into())),
            }
        }

        let u32_view: &[u32] = cast_slice(&self.raw_block_buf);
        let samples_per_block = block_bytes / 4;

        buf.reserve(samples_per_block * channels);

        for i in 0..samples_per_block {
            for ch in 0..channels {
                let src_index = (ch * samples_per_block) + i;
                let sample = if self.metadata.bps == 1 {
                    (u32_view[src_index] as i32).to_be()
                } else {
                    u32_view[src_index] as i32
                };
                buf.push_back(sample.reverse_bits());
            }
        }

        // remove metadata chunk
        self.read += total_bytes_to_read;
        if self.read >= self.metadata.data_chunk_size as usize {
            for _ in 0..(self.read - self.metadata.data_chunk_size as usize) {
                buf.pop_back();
            }
        }

        Ok(())
    }

    fn spec(&self) -> Option<MediaSpec> {
        Some(self.spec)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;
    use std::io::Cursor;

    fn push_u32(vec: &mut Vec<u8>, val: u32) {
        vec.extend_from_slice(&val.to_le_bytes());
    }
    fn push_u64(vec: &mut Vec<u8>, val: u64) {
        vec.extend_from_slice(&val.to_le_bytes());
    }

    fn create_full_mock_dsf(channels: u32, block_size: u32, data_payload: &[u8]) -> Vec<u8> {
        let mut mock = Vec::new();

        let dsd_chunk_len = 28u64;
        let fmt_chunk_len = 52u64;
        let data_header_len = 12u64;
        let data_payload_len = data_payload.len() as u64;
        let data_chunk_len = data_header_len + data_payload_len;

        let file_size = dsd_chunk_len + fmt_chunk_len + data_chunk_len;
        let metadata_ptr = 0u64;

        // ==========================
        // 1. 'DSD ' Chunk (28 bytes)
        // ==========================
        mock.extend_from_slice(b"DSD ");
        push_u64(&mut mock, dsd_chunk_len); // Chunk Size
        push_u64(&mut mock, file_size); // Total File Size
        push_u64(&mut mock, metadata_ptr); // Pointer to Metadata

        // ==========================
        // 2. 'fmt ' Chunk (52 bytes)
        // ==========================
        mock.extend_from_slice(b"fmt ");
        push_u64(&mut mock, fmt_chunk_len); // Chunk Size
        push_u32(&mut mock, 1); // Version
        push_u32(&mut mock, 0); // Format ID (0 = DSD Raw)
        push_u32(&mut mock, 2); // Channel Type (2 = Stereo)
        push_u32(&mut mock, channels); // Channel Num
        push_u32(&mut mock, 2822400); // Sample Freq (DSD64)
        push_u32(&mut mock, 8); // Bits per sample

        // Sample Count calculation: (bytes * 8) / channels
        let sample_count = (data_payload_len * 8) / channels as u64;
        push_u64(&mut mock, sample_count); // Sample Count

        push_u32(&mut mock, block_size); // Block Size Per Channel
        push_u32(&mut mock, 0); // Reserved

        // ==========================
        // 3. 'data' Chunk
        // ==========================
        mock.extend_from_slice(b"data");
        push_u64(&mut mock, data_chunk_len); // Chunk Size (Header + Audio)

        mock.extend_from_slice(data_payload);

        mock
    }

    #[test]
    fn test_dsd_reader_full_integration() {
        let block_size = 4;
        let channels = 2;

        let mut audio_data = Vec::new();

        // --- Block 1 ---
        // Left Channel (4 bytes): 0x01020304
        audio_data.extend_from_slice(&0x04030201u32.to_le_bytes());
        // Right Channel (4 bytes): 0x0A0B0C0D
        audio_data.extend_from_slice(&0x0D0C0B0Au32.to_le_bytes());

        // --- Block 2 ---
        // Left Channel: 0x55555555
        audio_data.extend_from_slice(&0x55555555u32.to_le_bytes());
        // Right Channel: 0x66666666
        audio_data.extend_from_slice(&0x66666666u32.to_le_bytes());

        let mock_file_bytes = create_full_mock_dsf(channels, block_size, &audio_data);

        let reader = Cursor::new(mock_file_bytes);

        let mut dsd = DsdDecoder::new(reader).expect("Header parsing failed");

        assert_eq!(dsd.metadata.channel_block_size, 4);

        // test Decode logic
        let mut output_buf = VecDeque::new();

        let result = dsd.decode(&mut output_buf);
        assert!(result.is_ok());

        // Block 1 Left (0x04030201)
        assert_eq!(
            output_buf.pop_front(),
            Some((0x04030201u32 as i32).reverse_bits())
        );
        // Block 1 Right (0x0D0C0B0A)
        assert_eq!(
            output_buf.pop_front(),
            Some((0x0D0C0B0Au32 as i32).reverse_bits())
        );

        let result = dsd.decode(&mut output_buf);
        assert!(result.is_ok());

        // Block 2 Left (0x55555555)
        assert_eq!(
            output_buf.pop_front(),
            Some((0x55555555u32 as i32).reverse_bits())
        );
        // Block 2 Right (0x66666666)
        assert_eq!(
            output_buf.pop_front(),
            Some((0x66666666u32 as i32).reverse_bits())
        );

        assert!(output_buf.is_empty());
    }

    #[test]
    fn test_pcm_decoder() {
        let mut mock_wav = Vec::new();
        let data: Vec<i16> = vec![1000, -1000, 2000, -2000];
        let data_bytes: Vec<u8> = data.iter().flat_map(|&s| s.to_le_bytes()).collect();
        let data_len = data_bytes.len() as u32;

        mock_wav.extend_from_slice(b"RIFF");
        mock_wav.extend_from_slice(&(36 + data_len).to_le_bytes());
        mock_wav.extend_from_slice(b"WAVE");
        mock_wav.extend_from_slice(b"fmt ");
        mock_wav.extend_from_slice(&16u32.to_le_bytes());
        mock_wav.extend_from_slice(&1u16.to_le_bytes()); // PCM
        mock_wav.extend_from_slice(&2u16.to_le_bytes()); // Stereo
        mock_wav.extend_from_slice(&44100u32.to_le_bytes());
        mock_wav.extend_from_slice(&(44100u32 * 4).to_le_bytes()); // ByteRate
        mock_wav.extend_from_slice(&4u16.to_le_bytes()); // BlockAlign
        mock_wav.extend_from_slice(&16u16.to_le_bytes()); // BitsPerSample
        mock_wav.extend_from_slice(b"data");
        mock_wav.extend_from_slice(&data_len.to_le_bytes());
        mock_wav.extend_from_slice(&data_bytes);

        let cursor = Cursor::new(mock_wav);
        let mut decoder =
            PcmDecoder::new(cursor, Some("wav")).expect("Failed to create PcmDecoder");

        let mut buf = VecDeque::new();
        decoder.decode(&mut buf).expect("Failed to decode");

        assert!(!buf.is_empty());
        // Symphonia scales 16-bit PCM to i32 by shifting left 16 bits
        assert_eq!(buf.pop_front(), Some((1000i32) << 16));
        assert_eq!(buf.pop_front(), Some((-1000i32) << 16));
        assert_eq!(buf.pop_front(), Some((2000i32) << 16));
        assert_eq!(buf.pop_front(), Some((-2000i32) << 16));
    }
}
