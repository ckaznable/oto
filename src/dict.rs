use anyhow::Result;
use flate2::bufread::GzDecoder;
use lindera::{
    dictionary::{Dictionary, DictionaryBuilder, Metadata, load_fs_dictionary},
    mode::Mode,
    segmenter::Segmenter,
    tokenizer::Tokenizer,
};
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use std::{
    io::{Cursor, Read},
    sync::{Arc, mpsc::Sender},
};
use tar::Archive;
use ureq::tls::{TlsConfig, TlsProvider};

use crate::{event::MatcherCommand, media::TrackMeta, shared::PROJ_DIRS};
use wana_kana::{ConvertJapanese, IsJapaneseChar};

const DL_URL: &str = "https://Lindera.dev/mecab-ipadic-2.7.0-20250920.tar.gz";
const DIR: &str = "mecab-ipadic-2.7.0-20250920";
const MD5: &str = "a95c409f12f1023fce8ef91f991ef042";

pub fn kanji_to_romaji(tx: Sender<MatcherCommand>, playlist: &[TrackMeta]) {
    #[cfg(feature = "dict-jp-embed")]
    let dictionary =
        lindera::dictionary::load_dictionary_temporary(lindera::dictionary::DictionaryKind::IPADIC)
            .map_err(|e| anyhow::anyhow!(e.to_string()));

    #[cfg(not(feature = "dict-jp-embed"))]
    let dictionary = download_and_build_dictionary();

    let dictionary = match dictionary {
        Ok(dictionary) => dictionary,
        Err(e) => {
            log::error!("{e:?}");
            return;
        }
    };

    let segmenter = Segmenter::new(Mode::Normal, dictionary, None);
    let tokenizer = Arc::new(Tokenizer::new(segmenter));
    log::debug!("japanese tokenizer ready");

    let processed_data: Vec<Option<String>> = playlist
        .par_iter()
        .map(|track| {
            let tokenizer = Arc::clone(&tokenizer);

            let text = format!(
                "{}-{}-{}",
                track.title.as_deref().unwrap_or(""),
                track.artist.as_deref().unwrap_or(""),
                track.album.name.as_deref().unwrap_or(""),
            );

            convert_to_romaji(&text, &tokenizer)
        })
        .collect();

    tx.send(MatcherCommand::KanjiToRomaji(processed_data)).ok();
}

#[allow(dead_code)]
fn download_and_build_dictionary() -> Result<Dictionary> {
    let input_path = PROJ_DIRS.data_dir().join("dict/input");
    let output_path = PROJ_DIRS.data_dir().join("dict/output");
    std::fs::create_dir_all(&input_path)?;
    std::fs::create_dir_all(&output_path)?;

    if !input_path.join(DIR).exists() {
        log::debug!("cache dict not found, downloading to cache dir");
        let config = ureq::config::Config::builder()
            .tls_config(
                TlsConfig::builder()
                    // requires the native-tls feature
                    .provider(TlsProvider::NativeTls)
                    .build(),
            )
            .build();

        let agent = config.new_agent();
        let response = agent.get(DL_URL).call()?;
        let (_, body) = response.into_parts();

        let mut buf: Vec<u8> = Vec::with_capacity(body.content_length().unwrap_or(1024) as usize);
        body.into_reader().read_to_end(&mut buf)?;
        let digest = md5::compute(&buf);
        let hash_string = format!("{:x}", digest);
        if hash_string != MD5 {
            return Err(anyhow::anyhow!(
                "download dict file error, md5 check failed"
            ));
        }

        log::debug!("unpack dict files");
        let tar = GzDecoder::new(Cursor::new(buf));
        let mut archive = Archive::new(tar);
        archive.unpack(&input_path)?;

        log::debug!("build dict to lindera format");
        let metadata_json = include_str!("../assets/metadata.json");
        let metadata: Metadata = serde_json::from_str(metadata_json)?;
        let builder = DictionaryBuilder::new(metadata);
        builder.build_dictionary(&input_path.join(DIR), &output_path)?
    }

    log::debug!("load dict from {output_path:?}");
    load_fs_dictionary(&output_path).map_err(|e| anyhow::anyhow!(e.to_string()))
}

fn convert_to_romaji(text: &str, tokenizer: &Tokenizer) -> Option<String> {
    if !text
        .chars()
        .any(|s| s.is_kanji() || s.is_hiragana() || s.is_katakana())
    {
        return None;
    }

    if !text.chars().any(|s| s.is_kanji()) {
        return Some(text.to_romaji());
    }

    let tokens = tokenizer.tokenize(text).unwrap_or_default();
    let mut reading = String::with_capacity(text.len() * 2);

    for mut token in tokens {
        let details = token.get_detail(7);
        if let Some(d) = details {
            reading.push_str(d);
        }
    }

    Some(reading.to_romaji())
}
