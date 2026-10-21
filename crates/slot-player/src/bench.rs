//! Where the player's work goes, stage by stage, on a real file:
//! `task perf:player FILE=song.mp3`. Each stage is timed over the whole file and said
//! as how many times faster than it plays, and as a share of one core while it plays. The
//! machine is not the device, so the shares are not the device's; how they compare is.

use std::fs::File;
use std::path::Path;
use std::time::{Duration, Instant};

use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::DecoderOptions;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

use crate::alsa::Dither;
use crate::decode::Decoder;
use crate::engine::limit;
use crate::level::Leveling;
use crate::track::Track;

/// The quickest of a few runs, to leave out what else the machine was doing.
fn quickest(mut run: impl FnMut() -> usize) -> (Duration, usize) {
    (0..3)
        .map(|_| {
            let began = Instant::now();
            let samples = run();
            (began.elapsed(), samples)
        })
        .min()
        .expect("runs")
}

fn say(stage: &str, took: Duration, played: f64) {
    let times = played / took.as_secs_f64();
    println!(
        "{stage:<40} {times:>7.0}x as fast as it plays, {:>5.2}% of a core",
        100.0 / times
    );
}

/// As the player decodes: a packet at a time, read out as stereo.
fn decoded(path: &Path) -> usize {
    let mut decoder = Decoder::open(path).unwrap();
    let mut out = Vec::with_capacity(4096 * 2);
    let mut n = 0;
    loop {
        out.clear();
        let got = decoder.read_stereo(4096, &mut out);
        n += got * 2;
        if got < 4096 {
            return n;
        }
    }
}

/// symphonia alone, a packet at a time into one buffer: as fast as decoding goes.
fn symphonia_blocks(path: &Path) -> usize {
    let source = MediaSourceStream::new(Box::new(File::open(path).unwrap()), Default::default());
    let probed = symphonia::default::get_probe()
        .format(
            &Hint::new(),
            source,
            &FormatOptions::default(),
            &MetadataOptions::default(),
        )
        .unwrap();
    let mut format = probed.format;
    let track = format.default_track().unwrap();
    let id = track.id;
    let mut decoder = crate::decode::codecs()
        .make(&track.codec_params, &DecoderOptions::default())
        .unwrap();
    let mut buf: Option<SampleBuffer<f32>> = None;
    let mut n = 0;
    while let Ok(packet) = format.next_packet() {
        if packet.track_id() != id {
            continue;
        }
        let Ok(decoded) = decoder.decode(&packet) else {
            continue;
        };
        let buf = buf
            .get_or_insert_with(|| SampleBuffer::new(decoded.capacity() as u64, *decoded.spec()));
        buf.copy_interleaved_ref(decoded);
        n += buf.samples().len();
    }
    n
}

/// The track as the writer reads it, at its own rate and speed: decoded to stereo, leveled.
fn track(path: &Path, leveling: Leveling) -> usize {
    let (mut track, _) = Track::open(path, 0.0, 1.0, leveling, |own| own).unwrap();
    let mut chunk = vec![0.0; 4096 * 2];
    let mut n = 0;
    loop {
        let got = track.read(&mut chunk);
        n += got;
        if got < chunk.len() {
            return n;
        }
    }
}

#[test]
#[ignore = "a measurement, on a file given: task perf:player FILE=song.mp3"]
fn stages() {
    let path = std::env::var("SLOT_PLAYER_FILE").expect("SLOT_PLAYER_FILE");
    let path = Path::new(&path);
    let rate = f64::from(Decoder::open(path).unwrap().rate());
    let (took, samples) = quickest(|| decoded(path));
    let played = samples as f64 / 2.0 / rate;
    println!("{}: {played:.0} s at {rate} Hz", path.display());
    let (took_alone, _) = quickest(|| symphonia_blocks(path));
    say("decode: symphonia alone", took_alone, played);
    say("decode: a packet at a time, to stereo", took, played);
    let (took, _) = quickest(|| track(path, Leveling::Off));
    say("track: decoded to stereo", took, played);
    let (took, _) = quickest(|| track(path, Leveling::Album));
    say("track: and leveled", took, played);
    // What the writer and the device do with each sample: headroom and the limiter, then
    // dither down to 16 bits.
    let stereo = vec![0.5f32; (played * rate) as usize * 2];
    let (took, _) = quickest(|| {
        let out: Vec<f32> = stereo.iter().map(|&s| limit(s * 0.79)).collect();
        out.len()
    });
    say("writer: headroom and limiter", took, played);
    let mut dither = Dither(0x9e37_79b9);
    let (took, _) = quickest(|| {
        let out: Vec<i16> = stereo.iter().map(|&s| dither.cut(s)).collect();
        out.len()
    });
    say("device: dither to 16 bits", took, played);
}
