#![warn(clippy::pedantic)]

use core::fmt;
use std::{fs, io, path::PathBuf};

use camino::{Utf8Path, Utf8PathBuf};
use clap::Parser;

const AUDIO_EXT: phf::Set<&'static str> = phf::phf_set! {
    // trash
    "mp3",

    // open codecs/containers
    "flac",
    "opus",
    "ape",
    "ogg",
    "mka",

    // apple stuff
    "aac",
    "alac",
    "m4a",
    "caf",

    // windows stuff
    "wma",
    "wav",
};

#[derive(Debug)]
struct Audiophile {
    order: u64,
    name: Utf8PathBuf,
}

enum NotAudiophile {
    NoExt,
    HasExtNoOrder,
}

impl TryFrom<Utf8PathBuf> for Audiophile {
    type Error = (Utf8PathBuf, NotAudiophile);

    fn try_from(v: Utf8PathBuf) -> Result<Self, Self::Error> {
        let Some(ext) = v.extension() else {
            return Err((v, NotAudiophile::NoExt));
        };

        if AUDIO_EXT.contains(ext) {
            let Some(fname) = v.file_name() else {
                return Err((v, NotAudiophile::HasExtNoOrder));
            };

            let mut num_idx = 0;

            for (idx, c) in fname.char_indices().chain([(fname.len(), '\x00')]) {
                num_idx = idx;
                if !c.is_ascii_digit() {
                    break;
                }
            }

            let Ok(order) = fname[..num_idx].parse() else {
                return Err((v, NotAudiophile::HasExtNoOrder));
            };

            Ok(Self { order, name: v })
        } else {
            Err((v, NotAudiophile::NoExt))
        }
    }
}

fn get_track<'a>(iter: impl Iterator<Item = (&'a str, &'a str)>) -> Option<u64> {
    for (k, v) in iter {
        if k.to_ascii_lowercase() == "track" {
            let i = v.split_once('/').map_or(v, |(n, _total)| n);
            if let Ok(track) = i.parse() {
                return Some(track);
            }
        }
    }

    None
}

impl Audiophile {
    fn parse_tags(file: Utf8PathBuf) -> io::Result<Result<Self, Utf8PathBuf>> {
        let parse = ffmpeg_next::format::input(&file)?;

        if let Some(track) = get_track(parse.metadata().iter()) {
            Ok(Ok(Self {
                name: file,
                order: track,
            }))
        } else {
            Ok(Err(file))
        }
    }
}

fn collect_audio_files(dir: &Utf8Path) -> io::Result<Vec<Audiophile>> {
    let mut res = vec![];
    let mut ffmpegd = false;

    for file in fs::read_dir(dir)? {
        let file = file?;

        if file.file_type()?.is_file() {
            let filename =
                Utf8PathBuf::try_from(PathBuf::from(file.file_name())).map_err(io::Error::other)?;

            match Audiophile::try_from(filename) {
                Ok(file) => res.push(file),
                Err((_, NotAudiophile::NoExt)) => (), // this isn't an audio file, ignore
                Err((buf, NotAudiophile::HasExtNoOrder)) => {
                    if !ffmpegd {
                        ffmpegd = true;
                        write_warn("initializing ffmpeg metadata fallback as filename contains no ordering");
                    }

                    match Audiophile::parse_tags(buf)? {
                        Ok(file) => res.push(file),
                        Err(buf) => write_warn(format_args!("Warn: tried to treat `{buf}` as an audio file, but it could not be ordered")),
                    }
                }
            }
        }
    }

    Ok(res)
}

fn write_warn(msg: impl fmt::Display) {
    use io::Write;
    let _ignore = writeln!(io::stderr().lock(), "\x1b[93mWARN:\x1b[0m {msg}");
}

/// A simple CLI to generate an m3u8 playlist from a cdrip'ed album
#[derive(clap::Parser)]
struct Args {
    /// The directory to scan as an album
    #[arg(default_value = ".")]
    directory: Utf8PathBuf,

    /// the filename to output to
    #[arg(short, long, default_value = "playlist.m3u8")]
    outfile: Utf8PathBuf,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    let mut data = collect_audio_files(&args.directory)?;

    data.sort_unstable_by_key(|i| i.order);

    let mut out = String::new();

    for af in data {
        use io::Write;

        let n = af.name.file_name().unwrap();

        writeln!(
            io::stdout(),
            "\x1b[37mwriting track \x1b[92m#{:02}\x1b[0m: {n}",
            af.order
        )?;

        out.reserve(n.len() + 1);
        out += n;
        out += "\n";
    }

    fs::write(args.outfile, out)?;

    Ok(())
}
