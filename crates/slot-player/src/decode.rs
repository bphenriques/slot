//! A file decoded a packet at a time, as symphonia decodes and as FFmpeg and Rockbox do: each
//! packet whole into one buffer, read from there, rather than a sample at a time. Set up as
//! rodio had it before: gapless, seeking to the sample, packets that will not decode skipped.

use std::fs::File;
use std::path::Path;
use std::sync::OnceLock;

use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::{self, CodecRegistry, DecoderOptions, CODEC_TYPE_NULL};
use symphonia::core::errors::Error;
use symphonia::core::formats::{FormatOptions, FormatReader, SeekMode, SeekTo};
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;
use symphonia::core::units::Time;

/// The codecs it decodes: symphonia's own, and Opus through libopus, which symphonia reads in Ogg
/// but has no decoder for.
pub(crate) fn codecs() -> &'static CodecRegistry {
    static CODECS: OnceLock<CodecRegistry> = OnceLock::new();
    CODECS.get_or_init(|| {
        let mut codecs = CodecRegistry::new();
        symphonia::default::register_enabled_codecs(&mut codecs);
        codecs.register_all::<symphonia_adapter_libopus::OpusDecoder>();
        codecs
    })
}

pub struct Decoder {
    format: Box<dyn FormatReader>,
    decoder: Box<dyn codecs::Decoder>,
    track: u32,
    /// The packet decoded last, interleaved, and how many of its samples have been read.
    packet: Option<SampleBuffer<f32>>,
    at: usize,
    channels: usize,
    rate: u32,
    /// Seconds long; 0 when that is not known.
    len: f64,
}

impl Decoder {
    pub fn open(path: &Path) -> Result<Decoder, String> {
        let file = File::open(path).map_err(|e| e.to_string())?;
        let source = MediaSourceStream::new(Box::new(file), Default::default());
        let mut hint = Hint::new();
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            hint.with_extension(ext);
        }
        let gapless = FormatOptions {
            enable_gapless: true,
            ..Default::default()
        };
        let probed = symphonia::default::get_probe()
            .format(&hint, source, &gapless, &MetadataOptions::default())
            .map_err(|e| e.to_string())?;
        let format = probed.format;
        let track = format
            .tracks()
            .iter()
            .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
            .ok_or("no track it can play")?;
        let params = &track.codec_params;
        let decoder = codecs()
            .make(params, &DecoderOptions::default())
            .map_err(|e| e.to_string())?;
        let len = params
            .time_base
            .zip(params.n_frames)
            .map_or(0.0, |(base, n)| seconds(base.calc_time(n)));
        let mut decoder = Decoder {
            track: track.id,
            channels: params.channels.map_or(0, |c| c.count()),
            rate: params.sample_rate.unwrap_or(0),
            format,
            decoder,
            packet: None,
            at: 0,
            len,
        };
        // What the file says of itself may be missing; its first packet says it surely.
        if !decoder.next_packet() {
            return Err("nothing in it to play".into());
        }
        Ok(decoder)
    }

    /// Its rate, as its packets have it.
    pub fn rate(&self) -> u32 {
        self.rate
    }

    pub fn len(&self) -> f64 {
        self.len
    }

    /// Moves to `secs` in, to the sample: to the packet before it, then on through what is
    /// before it there. Past the end is the end.
    pub fn seek(&mut self, secs: f64) -> Result<(), String> {
        let secs = if self.len > 0.0 {
            secs.min(self.len)
        } else {
            secs
        };
        let to = SeekTo::Time {
            time: Time::from(secs.max(0.0)),
            track_id: Some(self.track),
        };
        let landed = self
            .format
            .seek(SeekMode::Accurate, to)
            .map_err(|e| e.to_string())?;
        self.decoder.reset();
        (self.packet, self.at) = (None, 0);
        // On from where it landed to the frame asked, counted in frames from the start, as a
        // time rounded to its timestamp could be a frame short.
        let rate = f64::from(self.rate);
        let landed = match self.decoder.codec_params().time_base {
            Some(base) => (seconds(base.calc_time(landed.actual_ts)) * rate).round(),
            None => secs * rate,
        };
        self.skip(((secs * rate).round() - landed).max(0.0) as usize);
        Ok(())
    }

    /// Up to `frames` frames onto the end of `out`, as stereo, the first two channels or a mono
    /// one on both sides; and how many it got, fewer only at the end.
    pub fn read_stereo(&mut self, frames: usize, out: &mut Vec<f32>) -> usize {
        let mut got = 0;
        while got < frames {
            let Some(packet) = self.packet.as_ref().filter(|p| self.at < p.len()) else {
                if self.next_packet() {
                    continue;
                }
                break;
            };
            let channels = self.channels.max(1);
            let samples = &packet.samples()[self.at..];
            let take = (frames - got).min(samples.len() / channels);
            for frame in samples[..take * channels].chunks_exact(channels) {
                out.extend([frame[0], frame[channels.min(2) - 1]]);
            }
            self.at += take * channels;
            got += take;
        }
        got
    }

    /// Passes over `frames` frames.
    fn skip(&mut self, mut frames: usize) {
        while frames > 0 {
            let Some(packet) = self.packet.as_ref().filter(|p| self.at < p.len()) else {
                if self.next_packet() {
                    continue;
                }
                return;
            };
            let channels = self.channels.max(1);
            let take = frames.min((packet.len() - self.at) / channels);
            self.at += take * channels;
            frames -= take;
        }
    }

    /// Decodes the next packet with sound in it, or says there is none: the end, or a
    /// failure that ends it. One that will not decode is passed over.
    fn next_packet(&mut self) -> bool {
        loop {
            let Ok(packet) = self.format.next_packet() else {
                return false;
            };
            if packet.track_id() != self.track {
                continue;
            }
            let decoded = match self.decoder.decode(&packet) {
                Ok(decoded) => decoded,
                Err(Error::DecodeError(_)) => continue,
                Err(_) => return false,
            };
            if decoded.frames() == 0 {
                continue;
            }
            let spec = *decoded.spec();
            let room = decoded.capacity() * spec.channels.count();
            let mut buffer = match self.packet.take() {
                Some(b) if b.capacity() >= room => b,
                _ => SampleBuffer::new(decoded.capacity() as u64, spec),
            };
            buffer.copy_interleaved_ref(decoded);
            (self.packet, self.at) = (Some(buffer), 0);
            (self.channels, self.rate) = (spec.channels.count(), spec.rate);
            return true;
        }
    }
}

fn seconds(time: Time) -> f64 {
    time.seconds as f64 + time.frac
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A 16-bit WAV of `frames` frames, each channel's sample its frame's number, so where a
    /// sample came from can be read off it.
    fn counting(dir: &Path, channels: u16, frames: u32) -> std::path::PathBuf {
        let (rate, size) = (8000u32, frames * u32::from(channels) * 2);
        let mut b = Vec::new();
        b.extend(b"RIFF");
        b.extend((36 + size).to_le_bytes());
        b.extend(b"WAVEfmt ");
        b.extend(16u32.to_le_bytes());
        b.extend(1u16.to_le_bytes());
        b.extend(channels.to_le_bytes());
        b.extend(rate.to_le_bytes());
        b.extend((rate * u32::from(channels) * 2).to_le_bytes());
        b.extend((channels * 2).to_le_bytes());
        b.extend(16u16.to_le_bytes());
        b.extend(b"data");
        b.extend(size.to_le_bytes());
        for n in 0..frames {
            for c in 0..channels {
                b.extend(((n % 30000) as i16 + c as i16).to_le_bytes());
            }
        }
        let path = dir.join(format!("{channels}.wav"));
        std::fs::write(&path, b).unwrap();
        path
    }

    /// Frame numbers, as read off the left and right of what was decoded.
    fn numbers(stereo: &[f32]) -> Vec<(i32, i32)> {
        let n = |s: f32| (s * 32768.0).round() as i32;
        stereo.chunks(2).map(|f| (n(f[0]), n(f[1]))).collect()
    }

    #[test]
    fn read_in_any_lengths_it_is_every_frame_in_order_across_packets() {
        let dir = tempfile::tempdir().unwrap();
        let mut d = Decoder::open(&counting(dir.path(), 2, 20_000)).unwrap();
        assert_eq!((d.rate(), d.len()), (8000, 2.5));
        let mut out = Vec::new();
        for lengths in [1, 7, 333, 4096].iter().cycle() {
            if d.read_stereo(*lengths, &mut out) < *lengths {
                break;
            }
        }
        let read = numbers(&out);
        assert_eq!(read.len(), 20_000, "every frame, none twice");
        assert!(read
            .iter()
            .enumerate()
            .all(|(i, &f)| f == (i as i32, i as i32 + 1)));
    }

    #[test]
    fn a_seek_lands_on_the_very_frame_and_past_the_end_is_the_end() {
        let dir = tempfile::tempdir().unwrap();
        let mut d = Decoder::open(&counting(dir.path(), 2, 20_000)).unwrap();
        d.seek(1.25).unwrap();
        let mut out = Vec::new();
        d.read_stereo(3, &mut out);
        assert_eq!(
            numbers(&out),
            [(10_000, 10_001), (10_001, 10_002), (10_002, 10_003)]
        );
        d.seek(9.0).unwrap();
        assert_eq!(d.read_stereo(10, &mut Vec::new()), 0);
    }

    #[test]
    fn mono_is_on_both_sides_and_past_two_channels_only_the_first_two_play() {
        let dir = tempfile::tempdir().unwrap();
        for (channels, right) in [(1, 0), (4, 1)] {
            let mut d = Decoder::open(&counting(dir.path(), channels, 100)).unwrap();
            let mut out = Vec::new();
            d.read_stereo(2, &mut out);
            assert_eq!(
                numbers(&out),
                [(0, right), (1, 1 + right)],
                "{channels} channels"
            );
        }
    }

    #[test]
    fn an_opus_file_plays_at_48_khz_a_second_of_sound_and_seeks() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/tone.opus");
        let mut d = Decoder::open(&path).unwrap();
        assert_eq!(d.rate(), 48_000, "as Opus always decodes");
        let mut out = Vec::new();
        while d.read_stereo(4096, &mut out) > 0 {}
        let secs = out.len() as f64 / 2.0 / 48_000.0;
        assert!((0.95..1.1).contains(&secs), "a second long, not {secs}");
        let loud = out.iter().fold(0f32, |m, s| m.max(s.abs()));
        assert!(loud > 0.3, "the tone, not silence: {loud}");
        d.seek(0.5).unwrap();
        let mut rest = Vec::new();
        while d.read_stereo(4096, &mut rest) > 0 {}
        let left = rest.len() as f64 / 2.0 / 48_000.0;
        assert!((0.4..0.6).contains(&left), "half of it left, not {left}");
    }

    #[test]
    fn every_codec_the_library_s_files_may_hold_decodes() {
        use symphonia::core::codecs::{
            CODEC_TYPE_AAC, CODEC_TYPE_ALAC, CODEC_TYPE_FLAC, CODEC_TYPE_MP3, CODEC_TYPE_OPUS,
            CODEC_TYPE_PCM_S16LE, CODEC_TYPE_VORBIS,
        };
        let codecs = codecs();
        // An .m4a holds AAC or Apple Lossless; an .ogg Vorbis or FLAC; an .opus Opus.
        for codec in [
            CODEC_TYPE_MP3,
            CODEC_TYPE_AAC,
            CODEC_TYPE_ALAC,
            CODEC_TYPE_FLAC,
            CODEC_TYPE_VORBIS,
            CODEC_TYPE_OPUS,
            CODEC_TYPE_PCM_S16LE,
        ] {
            assert!(codecs.get_codec(codec).is_some(), "{codec}");
        }
    }

    #[test]
    fn a_file_with_nothing_to_play_will_not_open() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("x.mp3");
        std::fs::write(&path, b"not sound").unwrap();
        assert!(Decoder::open(&path).is_err());
    }
}
