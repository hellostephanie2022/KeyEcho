use std::{fs::File, path::Path, time::Duration};

use anyhow::{Context, Result};
use symphonia::{
    core::{
        audio::SampleBuffer,
        codecs::{CodecParameters, Decoder},
        formats::{FormatOptions, FormatReader, SeekMode, SeekTo},
        io::MediaSourceStream,
        probe::Hint,
        units::TimeBase,
    },
    default::{get_codecs, get_probe},
};

pub struct SoundDecoder {
    decoder: Box<dyn Decoder>,
    format: Box<dyn FormatReader>,
    time_base: TimeBase,
    pub(super) rate: u32,
    pub(super) channels: u16,
}

impl SoundDecoder {
    pub fn new<P>(path: P) -> Result<Self>
    where
        P: AsRef<Path>,
    {
        let file = File::open(&path)?;
        let mss = MediaSourceStream::new(Box::new(file), Default::default());

        let mut hint = Hint::new();
        if let Some(ext) = path.as_ref().extension().and_then(|p| p.to_str()) {
            hint.with_extension(ext);
        }

        let probe = get_probe().format(
            &hint,
            mss,
            &FormatOptions {
                // enable_gapless: true,
                ..Default::default()
            },
            &Default::default(),
        )?;

        let format = probe.format;
        let track = format.default_track().context("no default track")?;
        let decoder = get_codecs().make(&track.codec_params, &Default::default())?;

        let CodecParameters {
            sample_rate,
            channels,
            time_base,
            ..
        } = decoder.codec_params();

        let rate = sample_rate.context("no sample rate")?;
        let channels = channels.map(|c| c.count() as u16).context("no channels")?;
        let time_base = time_base.context("no time base")?;

        Ok(SoundDecoder {
            decoder,
            format,

            rate,
            channels,
            time_base,
        })
    }

    pub fn get_samples_buf(&mut self, start_ms: u64, duration_ms: u64) -> Result<Vec<i16>> {
        self.format.seek(
            SeekMode::Accurate,
            SeekTo::Time {
                track_id: None,
                time: Duration::from_millis(start_ms).into(),
            },
        )?;

        self.decoder.reset();

        let mut decoded_duration = 0u64;
        let mut samples_buffer = vec![];

        while decoded_duration < (duration_ms) {
            let packet = self.format.next_packet()?;

            let duration_time = self.time_base.calc_time(packet.dur);
            decoded_duration +=
                ((duration_time.seconds as f64 + duration_time.frac) * 1000.) as u64;

            let decoded = self.decoder.decode(&packet)?;
            let mut sample_buffer =
                SampleBuffer::<i16>::new(decoded.capacity() as u64, *decoded.spec());
            sample_buffer.copy_interleaved_ref(decoded);

            samples_buffer.extend_from_slice(sample_buffer.samples());
        }

        Ok(samples_buffer)
    }
}
