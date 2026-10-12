//! CPAL owns platform device access. DSP, channel mapping and bounded transport are ours.
use crate::{
    audio::{AudioProcessor, DspSettings},
    audio_queue::{self, Consumer, Producer},
    backend::{AudioBackend, AudioStream, Result},
    pitch::{Sample, LATENCY},
    RATE,
};
use cpal::{
    traits::{DeviceTrait, HostTrait, StreamTrait},
    SampleFormat, StreamConfig, SupportedBufferSize, SupportedStreamConfigRange,
};
use std::sync::{
    atomic::{AtomicBool, AtomicU32, AtomicU64, AtomicUsize, Ordering},
    Arc,
};
use std::time::{Duration, Instant};

const UNDERRUN: u32 = 1;
const OVERRUN: u32 = 2;
const INVALID_BUFFER: u32 = 4;
const DEVICE_ERROR: u32 = 8;

pub(crate) struct CpalBackend {
    output_device: String,
}
impl CpalBackend {
    pub(crate) fn new(output_device: &str) -> Self {
        Self {
            output_device: output_device.into(),
        }
    }
}

fn devices(input: bool) -> Result<Vec<cpal::Device>> {
    let host = cpal::default_host();
    Ok(if input {
        host.input_devices()?.collect()
    } else {
        host.output_devices()?.collect()
    })
}
fn device_label(device: &cpal::Device) -> Result<String> {
    Ok(format!(
        "{} [{}]",
        device.description()?.name(),
        device.id()?
    ))
}
fn device_labels(input: bool) -> Result<Vec<String>> {
    devices(input)?.iter().map(device_label).collect()
}
fn select_device(input: bool, name: &str) -> Result<cpal::Device> {
    if name.is_empty() {
        let host = cpal::default_host();
        return (if input {
            host.default_input_device()
        } else {
            host.default_output_device()
        })
        .ok_or_else(|| {
            format!(
                "No default {} device",
                if input { "input" } else { "output" }
            )
            .into()
        });
    }
    let mut matches = Vec::new();
    for device in devices(input)? {
        if name == device.description()?.name()
            || name == device_label(&device)?
            || name == device.id()?.to_string()
        {
            matches.push(device);
        }
    }
    match matches.len() {
        1 => Ok(matches.remove(0)),
        0 => Err(format!("Audio device '{name}' not found; use --list-devices").into()),
        _ => {
            Err(format!("Ambiguous device '{name}'; use the full label from --list-devices").into())
        }
    }
}

fn usable(config: &SupportedStreamConfigRange, frames: u32, format: SampleFormat) -> bool {
    config.channels() > 0
        && config.sample_format() == format
        && config.min_sample_rate() <= RATE
        && config.max_sample_rate() >= RATE
        && match config.buffer_size() {
            SupportedBufferSize::Range { min, max } => (*min..=*max).contains(&frames),
            SupportedBufferSize::Unknown => true,
        }
}
fn negotiate(
    input: &[SupportedStreamConfigRange],
    output: &[SupportedStreamConfigRange],
    frames: u32,
) -> Result<(SampleFormat, StreamConfig, StreamConfig)> {
    if !(1..=8192).contains(&frames) {
        return Err("Frames must be 1..8192".into());
    }
    // A common typed format preserves neutral samples without a lossy intermediate cast.
    for format in [SampleFormat::F32, SampleFormat::I32, SampleFormat::I16] {
        let best = |configs: &[SupportedStreamConfigRange]| {
            configs
                .iter()
                .filter(|c| usable(c, frames, format))
                .min_by_key(|c| c.channels())
                .cloned()
        };
        if let (Some(input), Some(output)) = (best(input), best(output)) {
            let convert = |range: SupportedStreamConfigRange| {
                let buffer_size = match range.buffer_size() {
                    SupportedBufferSize::Range { .. } => cpal::BufferSize::Fixed(frames),
                    SupportedBufferSize::Unknown => cpal::BufferSize::Default,
                };
                let mut config = range.with_sample_rate(RATE).config();
                config.buffer_size = buffer_size;
                config
            };
            return Ok((format, convert(input), convert(output)));
        }
    }
    Err("Input and output need a common f32/i32/i16 format at 48000 Hz and a supported period. No implicit resampling or format conversion is performed.".into())
}

struct Prepared {
    input: cpal::Device,
    output: cpal::Device,
    format: SampleFormat,
    input_config: StreamConfig,
    output_config: StreamConfig,
    details: String,
}
impl CpalBackend {
    fn prepare(&self, input_name: &str, frames: u32) -> Result<Prepared> {
        let input = select_device(true, input_name)?;
        let output = select_device(false, &self.output_device)?;
        let input_ranges: Vec<_> = input.supported_input_configs()?.collect();
        let output_ranges: Vec<_> = output.supported_output_configs()?.collect();
        let (format, input_config, output_config) =
            negotiate(&input_ranges, &output_ranges, frames)?;
        let details = format!("CPAL {:?}: input '{}' / {} channels / {:?}; output '{}' / {} channels / {:?}; 48000 Hz; format {format:?}. Input 1 -> output 1, other outputs silent. Default periods are chosen by the host and observed during playback.",
            cpal::default_host().id(), input.description()?.name(), input_config.channels, input_config.buffer_size,
            output.description()?.name(), output_config.channels, output_config.buffer_size);
        Ok(Prepared {
            input,
            output,
            format,
            input_config,
            output_config,
            details,
        })
    }
}
impl AudioBackend for CpalBackend {
    fn device_names(&self) -> Result<Vec<String>> {
        device_labels(true)
    }
    fn output_device_names(&self) -> Result<Vec<String>> {
        device_labels(false)
    }
    fn inspect(&self, name: &str, frames: u32) -> Result<String> {
        Ok(self.prepare(name, frames)?.details)
    }
    fn open(&self, name: &str, frames: u32, dsp: DspSettings) -> Result<Box<dyn AudioStream>> {
        let prepared = self.prepare(name, frames)?;
        match prepared.format {
            SampleFormat::F32 => build::<f32>(prepared, frames, dsp),
            SampleFormat::I32 => build::<i32>(prepared, frames, dsp),
            SampleFormat::I16 => build::<i16>(prepared, frames, dsp),
            _ => Err("Unsupported negotiated sample format".into()),
        }
    }
}

struct Metrics {
    active: AtomicBool,
    faults: AtomicU32,
    input_blocks: AtomicU64,
    output_blocks: AtomicU64,
    played_frames: AtomicU64,
    nonzero: AtomicU64,
    underruns: AtomicU64,
    overruns: AtomicU64,
    max_input_ns: AtomicU64,
    max_output_ns: AtomicU64,
    deadline_misses: AtomicU64,
    max_input_frames: AtomicUsize,
    max_output_frames: AtomicUsize,
    queue_depth: AtomicUsize,
    queue_peak: AtomicUsize,
    prime_frames: AtomicUsize,
    last_input_ns: AtomicU64,
    last_output_ns: AtomicU64,
    origin: Instant,
}
impl Metrics {
    fn new(prime_frames: usize) -> Self {
        Self {
            active: AtomicBool::new(false),
            faults: AtomicU32::new(0),
            input_blocks: AtomicU64::new(0),
            output_blocks: AtomicU64::new(0),
            played_frames: AtomicU64::new(0),
            nonzero: AtomicU64::new(0),
            underruns: AtomicU64::new(0),
            overruns: AtomicU64::new(0),
            max_input_ns: AtomicU64::new(0),
            max_output_ns: AtomicU64::new(0),
            deadline_misses: AtomicU64::new(0),
            max_input_frames: AtomicUsize::new(0),
            max_output_frames: AtomicUsize::new(0),
            queue_depth: AtomicUsize::new(0),
            queue_peak: AtomicUsize::new(0),
            prime_frames: AtomicUsize::new(prime_frames),
            last_input_ns: AtomicU64::new(0),
            last_output_ns: AtomicU64::new(0),
            origin: Instant::now(),
        }
    }
    fn now_ns(&self) -> u64 {
        self.origin.elapsed().as_nanos().min(u64::MAX as u128) as u64
    }
    fn processing(&self) -> bool {
        self.active.load(Ordering::Acquire) && self.faults.load(Ordering::Relaxed) == 0
    }
}

struct Capture<T: Sample> {
    processor: AudioProcessor<T>,
    mono: Vec<T>,
    processed: Vec<T>,
    channels: usize,
    queue: Producer<T>,
    metrics: Arc<Metrics>,
}
impl<T: Sample> Capture<T> {
    fn process(&mut self, data: &[T]) {
        if !self.metrics.processing() || data.is_empty() {
            return;
        }
        let start = Instant::now();
        if !data.len().is_multiple_of(self.channels) {
            self.metrics
                .faults
                .fetch_or(INVALID_BUFFER, Ordering::Relaxed);
            return;
        }
        let frames = data.len() / self.channels;
        self.metrics
            .max_input_frames
            .fetch_max(frames, Ordering::Relaxed);
        // Never drop or overwrite queued speech. An xrun terminates this session.
        if frames > self.queue.free() {
            self.metrics.overruns.fetch_add(1, Ordering::Relaxed);
            self.metrics.faults.fetch_or(OVERRUN, Ordering::Relaxed);
            return;
        }
        for chunk in data.chunks(self.mono.len() * self.channels) {
            let count = chunk.len() / self.channels;
            for (sample, frame) in self.mono[..count]
                .iter_mut()
                .zip(chunk.chunks_exact(self.channels))
            {
                *sample = frame[0];
            }
            let nonzero = self
                .processor
                .process_block(&self.mono[..count], &mut self.processed[..count]);
            let Ok(nonzero) = nonzero else {
                self.metrics
                    .faults
                    .fetch_or(INVALID_BUFFER, Ordering::Relaxed);
                return;
            };
            self.metrics.nonzero.fetch_add(nonzero, Ordering::Relaxed);
            #[cfg(windows)]
            crate::spectrum::push_output_samples(&self.processed[..count]);
            if !self.queue.push_block(&self.processed[..count]) {
                self.metrics.faults.fetch_or(OVERRUN, Ordering::Relaxed);
                return;
            }
        }
        self.metrics.input_blocks.fetch_add(1, Ordering::Relaxed);
        self.metrics
            .last_input_ns
            .store(self.metrics.now_ns(), Ordering::Relaxed);
        let elapsed = start.elapsed().as_nanos().min(u64::MAX as u128) as u64;
        self.metrics
            .max_input_ns
            .fetch_max(elapsed, Ordering::Relaxed);
        if elapsed > frames as u64 * 1_000_000_000 / RATE as u64 {
            self.metrics.deadline_misses.fetch_add(1, Ordering::Relaxed);
        }
    }
}

struct Playback<T> {
    channels: usize,
    queue: Consumer<T>,
    metrics: Arc<Metrics>,
    primed: bool,
}
impl<T: Sample> Playback<T> {
    fn process(&mut self, data: &mut [T]) {
        let start = Instant::now();
        data.fill(T::default());
        if !self.metrics.processing() || data.is_empty() {
            return;
        }
        if !data.len().is_multiple_of(self.channels) {
            self.metrics
                .faults
                .fetch_or(INVALID_BUFFER, Ordering::Relaxed);
            return;
        }
        let frames = data.len() / self.channels;
        self.metrics
            .max_output_frames
            .fetch_max(frames, Ordering::Relaxed);
        self.metrics.output_blocks.fetch_add(1, Ordering::Relaxed);
        self.metrics
            .last_output_ns
            .store(self.metrics.now_ns(), Ordering::Relaxed);
        let queued = self.queue.len();
        self.metrics.queue_peak.fetch_max(queued, Ordering::Relaxed);
        if !self.primed {
            let target = self
                .metrics
                .prime_frames
                .load(Ordering::Relaxed)
                .max(frames.saturating_mul(2));
            self.metrics.prime_frames.store(target, Ordering::Relaxed);
            self.metrics.queue_depth.store(queued, Ordering::Relaxed);
            if target > self.queue.capacity() {
                self.metrics
                    .faults
                    .fetch_or(INVALID_BUFFER, Ordering::Relaxed);
                return;
            }
            if queued < target {
                return;
            }
            self.primed = true;
        }
        if queued < frames {
            self.metrics.underruns.fetch_add(1, Ordering::Relaxed);
            self.metrics.faults.fetch_or(UNDERRUN, Ordering::Relaxed);
            return;
        }
        for frame in data.chunks_exact_mut(self.channels) {
            // The unique consumer already checked availability; producer only adds samples.
            frame[0] = self.queue.pop().unwrap_or_default();
        }
        let elapsed = start.elapsed().as_nanos().min(u64::MAX as u128) as u64;
        self.metrics
            .max_output_ns
            .fetch_max(elapsed, Ordering::Relaxed);
        if elapsed > frames as u64 * 1_000_000_000 / RATE as u64 {
            self.metrics.deadline_misses.fetch_add(1, Ordering::Relaxed);
        }
        self.metrics
            .played_frames
            .fetch_add(frames as u64, Ordering::Relaxed);
        self.metrics
            .queue_depth
            .store(self.queue.len(), Ordering::Relaxed);
    }
}

fn build<T: Sample + cpal::SizedSample + Send + 'static>(
    prepared: Prepared,
    frames: u32,
    dsp: DspSettings,
) -> Result<Box<dyn AudioStream>> {
    let capacity = (frames as usize * 8).max(32768).next_power_of_two();
    let (producer, consumer) = audio_queue::channel::<T>(capacity);
    let metrics = Arc::new(Metrics::new(frames as usize * 2));
    let mut capture = Capture {
        processor: AudioProcessor::new(RATE, frames as usize, dsp)?,
        mono: vec![T::default(); frames as usize],
        processed: vec![T::default(); frames as usize],
        channels: prepared.input_config.channels as usize,
        queue: producer,
        metrics: metrics.clone(),
    };
    let mut playback = Playback {
        channels: prepared.output_config.channels as usize,
        queue: consumer,
        metrics: metrics.clone(),
        primed: false,
    };
    let errors = metrics.clone();
    let input = prepared.input.build_input_stream(
        prepared.input_config,
        move |data: &[T], _| capture.process(data),
        move |_| {
            errors.faults.fetch_or(DEVICE_ERROR, Ordering::Relaxed);
        },
        Some(Duration::from_secs(5)),
    )?;
    let errors = metrics.clone();
    let output = prepared.output.build_output_stream(
        prepared.output_config,
        move |data: &mut [T], _| playback.process(data),
        move |_| {
            errors.faults.fetch_or(DEVICE_ERROR, Ordering::Relaxed);
        },
        Some(Duration::from_secs(5)),
    )?;
    Ok(Box::new(CpalStream {
        output: Some(output),
        input: Some(input),
        metrics,
        details: prepared.details,
        capacity,
        started: false,
        running: false,
        started_at: None,
    }))
}

struct CpalStream {
    output: Option<cpal::Stream>,
    input: Option<cpal::Stream>,
    metrics: Arc<Metrics>,
    details: String,
    capacity: usize,
    started: bool,
    running: bool,
    started_at: Option<Instant>,
}
impl AudioStream for CpalStream {
    fn start(&mut self) -> Result<()> {
        if self.started {
            return Err("Open a new CPAL stream after stopping".into());
        }
        let (Some(output), Some(input)) = (&self.output, &self.input) else {
            return Err("CPAL stream has been released; open a new stream".into());
        };
        self.started = true;
        self.metrics.active.store(true, Ordering::Release);
        if let Err(error) = output.play().and_then(|_| input.play()) {
            self.stop()?;
            return Err(error.into());
        }
        self.running = true;
        self.started_at = Some(Instant::now());
        Ok(())
    }
    fn stop(&mut self) -> Result<()> {
        self.metrics.active.store(false, Ordering::Release);
        // Some ALSA devices cannot pause. Dropping both streams works on every
        // CPAL host and joins/releases callbacks before their storage is freed.
        self.running = false;
        self.input.take();
        self.output.take();
        Ok(())
    }
    fn blocks(&self) -> u64 {
        self.metrics.input_blocks.load(Ordering::Relaxed)
    }
    fn has_playback(&self) -> bool {
        self.metrics.played_frames.load(Ordering::Relaxed) > 0
    }
    fn summary(&self) -> String {
        let m = &self.metrics;
        format!("CPAL input/output callbacks={}/{}; nonzero input samples={}; max input/output callback={:.3}/{:.3} us; deadline misses={}; observed max input/output period={}/{} frames; queue now/peak={}/{} frames; underruns={}; overruns={}; fault flags=0x{:x}\n{}",
            self.blocks(), m.output_blocks.load(Ordering::Relaxed), m.nonzero.load(Ordering::Relaxed),
            m.max_input_ns.load(Ordering::Relaxed) as f64 / 1000.0, m.max_output_ns.load(Ordering::Relaxed) as f64 / 1000.0, m.deadline_misses.load(Ordering::Relaxed),
            m.max_input_frames.load(Ordering::Relaxed), m.max_output_frames.load(Ordering::Relaxed),
            m.queue_depth.load(Ordering::Relaxed), m.queue_peak.load(Ordering::Relaxed),
            m.underruns.load(Ordering::Relaxed), m.overruns.load(Ordering::Relaxed), m.faults.load(Ordering::Relaxed), self.details)
    }
    fn latency_summary(&self) -> Result<String> {
        let target = self.metrics.prime_frames.load(Ordering::Relaxed);
        let queued = self.metrics.queue_depth.load(Ordering::Relaxed);
        Ok(format!("CPAL driver latency unavailable through this adapter; queue startup target={target} frames ({:.3} ms), queued snapshot={queued} frames ({:.3} ms), capacity={} frames; wet DSP={LATENCY} samples ({:.3} ms), neutral DSP=0. Hardware round-trip latency is unmeasured. Separate device clock drift is not corrected.",
            target as f64 * 1000.0 / RATE as f64, queued as f64 * 1000.0 / RATE as f64, self.capacity,
            LATENCY as f64 * 1000.0 / RATE as f64))
    }
    fn check_events(&self) -> Result<()> {
        let faults = self.metrics.faults.load(Ordering::Relaxed);
        if faults != 0 {
            return Err(format!("CPAL audio stopped (flags 0x{faults:x}: underrun=1,overrun=2,invalid buffer=4,device error=8). Check device connection, period and input/output clock drift; restart after adjusting settings.").into());
        }
        if self.running
            && self
                .started_at
                .is_some_and(|time| time.elapsed() > Duration::from_secs(2))
        {
            let now = self.metrics.now_ns();
            if now.saturating_sub(self.metrics.last_input_ns.load(Ordering::Relaxed))
                > 2_000_000_000
                || now.saturating_sub(self.metrics.last_output_ns.load(Ordering::Relaxed))
                    > 2_000_000_000
            {
                return Err("No CPAL input or output callbacks for 2 seconds".into());
            }
        }
        Ok(())
    }
    fn check_test_result(&self) -> Result<()> {
        self.check_events()?;
        if self.blocks() == 0 || self.metrics.output_blocks.load(Ordering::Relaxed) == 0 {
            return Err("No CPAL input/output callbacks observed".into());
        }
        if !self.has_playback() {
            return Err("No processed samples reached CPAL output; queue was still priming".into());
        }
        if self.metrics.deadline_misses.load(Ordering::Relaxed) != 0 {
            return Err("CPAL test had callback deadline misses".into());
        }
        Ok(())
    }
}
impl Drop for CpalStream {
    fn drop(&mut self) {
        // Both closures own their buffers and queue endpoints until CPAL drops the streams.
        let _ = self.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::Options;

    fn callbacks<T: Sample + Send>(
        max_frames: usize,
        channels: usize,
        args: &[&str],
    ) -> (Capture<T>, Playback<T>, Arc<Metrics>) {
        let options = Options::parse(args.iter().map(|s| s.to_string())).unwrap();
        let (producer, consumer) =
            audio_queue::channel((max_frames * 8).max(32).next_power_of_two());
        let metrics = Arc::new(Metrics::new(max_frames * 2));
        metrics.active.store(true, Ordering::Release);
        (
            Capture {
                processor: AudioProcessor::new(RATE, max_frames, DspSettings::from(&options))
                    .unwrap(),
                mono: vec![T::default(); max_frames],
                processed: vec![T::default(); max_frames],
                channels,
                queue: producer,
                metrics: metrics.clone(),
            },
            Playback {
                channels: 3,
                queue: consumer,
                metrics: metrics.clone(),
                primed: false,
            },
            metrics,
        )
    }

    fn mapping<T: Sample + Send + std::fmt::Debug>(input: &[T]) -> Vec<T> {
        let (mut capture, mut playback, metrics) = callbacks::<T>(3, 2, &[]);
        let mut startup = [T::default(); 6];
        playback.process(&mut startup);
        assert_eq!(metrics.underruns.load(Ordering::Relaxed), 0);
        let interleaved: Vec<_> = input.iter().flat_map(|&s| [s, input[0]]).collect();
        capture.process(&interleaved); // A host block bigger than the preallocated DSP block.
        let mut output = Vec::new();
        for frames in [2, 3, 3] {
            let mut data = vec![input[0]; frames * 3];
            playback.process(&mut data);
            for frame in data.as_chunks::<3>().0 {
                output.push(frame[0]);
                assert_eq!(&frame[1..], &[T::default(); 2]);
            }
        }
        assert_eq!(input, output);
        assert_eq!(metrics.faults.load(Ordering::Relaxed), 0);
        output
    }

    #[test]
    fn bypass_mapping_preserves_all_formats_and_silences_other_outputs() {
        mapping(&[i16::MIN, -1, 0, 1, i16::MAX, 7, 13, -12]);
        mapping(&[i32::MIN, -1, 0, 1, i32::MAX, 7, 13, -12]);
        let input = [
            0.5_f32,
            -0.0,
            f32::from_bits(1),
            -0.25,
            1.0,
            -1.0,
            0.125,
            0.0,
        ];
        let output = mapping(&input);
        assert_eq!(
            input.map(f32::to_bits).to_vec(),
            output.iter().map(|s| s.to_bits()).collect::<Vec<_>>()
        );
    }

    #[test]
    fn xruns_do_not_overwrite_or_repeat_speech_and_faults_mute_following_callbacks() {
        let (mut capture, mut playback, metrics) = callbacks::<f32>(3, 1, &[]);
        capture.process(&[0.25; 6]);
        playback.process(&mut [0.0; 6]);
        let mut output = [1.0; 15];
        playback.process(&mut output); // Needs five samples, but four remain.
        assert_eq!(output, [0.0; 15]);
        assert_eq!(metrics.faults.load(Ordering::Relaxed), UNDERRUN);
        assert_eq!(metrics.underruns.load(Ordering::Relaxed), 1);
        assert_eq!(playback.queue.len(), 4);
        capture.process(&[0.8; 3]);
        assert_eq!(playback.queue.len(), 4);

        let (mut capture, playback, metrics) = callbacks::<f32>(3, 1, &[]);
        capture.process(&[0.5; 33]);
        assert_eq!(metrics.faults.load(Ordering::Relaxed), OVERRUN);
        assert_eq!(metrics.overruns.load(Ordering::Relaxed), 1);
        assert_eq!(playback.queue.len(), 0);
    }

    #[test]
    fn malformed_or_inactive_callbacks_do_not_process_samples() {
        let (mut capture, mut playback, metrics) = callbacks::<f32>(3, 2, &[]);
        metrics.active.store(false, Ordering::Release);
        capture.process(&[0.5; 12]);
        let mut output = [1.0; 6];
        playback.process(&mut output);
        assert_eq!(output, [0.0; 6]);
        assert_eq!(playback.queue.len(), 0);
        metrics.active.store(true, Ordering::Release);
        capture.process(&[0.5; 3]);
        assert_eq!(metrics.faults.load(Ordering::Relaxed), INVALID_BUFFER);
        let (_, mut playback, metrics) = callbacks::<f32>(3, 1, &[]);
        playback.process(&mut [1.0; 5]);
        assert_eq!(metrics.faults.load(Ordering::Relaxed), INVALID_BUFFER);
    }

    fn range(
        format: SampleFormat,
        channels: u16,
        rate: u32,
        buffer: SupportedBufferSize,
    ) -> SupportedStreamConfigRange {
        SupportedStreamConfigRange::new(channels, rate, rate, buffer, format)
    }
    #[test]
    fn negotiation_validates_common_format_rate_and_period() {
        let known = SupportedBufferSize::Range { min: 64, max: 512 };
        let input = [
            range(SampleFormat::F32, 2, RATE, known),
            range(SampleFormat::I16, 1, RATE, known),
        ];
        let output = [range(
            SampleFormat::I16,
            2,
            RATE,
            SupportedBufferSize::Unknown,
        )];
        let (format, input_config, output_config) = negotiate(&input, &output, 128).unwrap();
        assert_eq!(format, SampleFormat::I16);
        assert_eq!(input_config.channels, 1);
        assert_eq!(input_config.buffer_size, cpal::BufferSize::Fixed(128));
        assert_eq!(output_config.buffer_size, cpal::BufferSize::Default);
        assert!(negotiate(&input, &output, 32).is_err());
        assert!(negotiate(&input, &output, 0).is_err());
        assert!(negotiate(&input, &[range(SampleFormat::I16, 2, 44100, known)], 128).is_err());
        assert!(negotiate(&input, &[range(SampleFormat::U16, 2, RATE, known)], 128).is_err());
    }

    #[test]
    fn wet_callbacks_match_shared_dsp_across_interleaving_and_variable_host_blocks() {
        let args = ["--pitch", "3", "--gate", "on", "--auto-level", "on"];
        let input: Vec<f32> = (0..12000).map(|i| (i as f32 * 0.025).sin() * 0.1).collect();
        let options = Options::parse(args.map(String::from)).unwrap();
        let mut reference = AudioProcessor::new(RATE, 512, DspSettings::from(&options)).unwrap();
        let mut expected = vec![0.0; input.len()];
        for (a, b) in input.chunks(512).zip(expected.chunks_mut(512)) {
            reference.process_block(a, b).unwrap();
        }
        let (mut capture, mut playback, metrics) = callbacks::<f32>(64, 2, &args);
        let mut actual = Vec::new();
        let mut offset = 0;
        for count in [1, 127, 13, 37].into_iter().cycle() {
            let end = (offset + count).min(input.len());
            let interleaved: Vec<_> = input[offset..end]
                .iter()
                .flat_map(|&sample| [sample, 0.9])
                .collect();
            capture.process(&interleaved);
            while playback.queue.len() >= if playback.primed { 64 } else { 128 } {
                let mut output = [0.0; 64 * 3];
                playback.process(&mut output);
                actual.extend(output.as_chunks::<3>().0.iter().map(|frame| frame[0]));
            }
            offset = end;
            if end == input.len() {
                break;
            }
        }
        let remaining = playback.queue.len();
        let mut tail = vec![0.0; remaining * 3];
        playback.process(&mut tail);
        actual.extend(tail.as_chunks::<3>().0.iter().map(|frame| frame[0]));
        assert_eq!(metrics.faults.load(Ordering::Relaxed), 0);
        assert_eq!(
            expected.iter().map(|s| s.to_bits()).collect::<Vec<_>>(),
            actual.iter().map(|s| s.to_bits()).collect::<Vec<_>>()
        );
    }

    use std::{
        alloc::{GlobalAlloc, Layout, System},
        cell::Cell,
    };
    struct CountingAllocator;
    thread_local! {
        static COUNTING: Cell<bool> = const { Cell::new(false) };
        static ALLOCATIONS: Cell<usize> = const { Cell::new(0) };
    }
    // SAFETY: allocations are delegated unchanged; thread-local counters isolate tests.
    unsafe impl GlobalAlloc for CountingAllocator {
        unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
            if COUNTING.try_with(Cell::get).unwrap_or(false) {
                let _ = ALLOCATIONS.try_with(|count| count.set(count.get() + 1));
            }
            unsafe { System.alloc(layout) }
        }
        unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
            unsafe {
                System.dealloc(pointer, layout);
            }
        }
        unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, size: usize) -> *mut u8 {
            if COUNTING.try_with(Cell::get).unwrap_or(false) {
                let _ = ALLOCATIONS.try_with(|count| count.set(count.get() + 1));
            }
            unsafe { System.realloc(pointer, layout, size) }
        }
    }
    #[global_allocator]
    static ALLOCATOR: CountingAllocator = CountingAllocator;

    #[test]
    fn capture_dsp_queue_and_playback_allocate_nothing() {
        let (mut capture, mut playback, metrics) = callbacks::<f32>(
            64,
            2,
            &["--pitch", "3", "--gate", "on", "--auto-level", "on"],
        );
        let input = [0.1; 128];
        let mut output = [0.0; 192];
        ALLOCATIONS.with(|count| count.set(0));
        COUNTING.with(|enabled| enabled.set(true));
        capture.process(&input);
        for _ in 0..128 {
            capture.process(&input);
            playback.process(&mut output);
        }
        COUNTING.with(|enabled| enabled.set(false));
        assert_eq!(ALLOCATIONS.with(Cell::get), 0);
        assert_eq!(metrics.faults.load(Ordering::Relaxed), 0);
    }
}
