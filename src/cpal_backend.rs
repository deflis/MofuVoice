//! CPAL owns platform device access. DSP, channel mapping and bounded transport are ours.
#[cfg(test)]
use crate::RATE;
use crate::{
    audio::{AudioProcessor, DspSettings},
    audio_queue::{self, Consumer, Producer},
    backend::{AudioBackend, AudioStream, ChannelRouting, Result},
    dsp::Sample,
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
    sample_rate: u32,
}
impl CpalBackend {
    pub(crate) fn new(output_device: &str, sample_rate: u32) -> Self {
        Self {
            output_device: output_device.into(),
            sample_rate,
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

fn usable(
    config: &SupportedStreamConfigRange,
    frames: u32,
    format: SampleFormat,
    rate: u32,
) -> bool {
    config.channels() > 0
        && config.sample_format() == format
        && config.min_sample_rate() <= rate
        && config.max_sample_rate() >= rate
        && match config.buffer_size() {
            SupportedBufferSize::Range { min, max } => (*min..=*max).contains(&frames),
            SupportedBufferSize::Unknown => true,
        }
}
fn negotiate_at_rate(
    input: &[SupportedStreamConfigRange],
    output: &[SupportedStreamConfigRange],
    frames: u32,
    rate: u32,
) -> Result<(SampleFormat, SampleFormat, StreamConfig, StreamConfig)> {
    crate::validate_sample_rate(rate)?;
    if !(1..=8192).contains(&frames) {
        return Err("Frames must be 1..8192".into());
    }
    let select = |ranges: &[SupportedStreamConfigRange], direction: &str| {
        let mut choices = Vec::new();
        for range in ranges {
            let low = range.min_sample_rate().max(8000);
            let high = range.max_sample_rate().min(192000);
            if low > high {
                continue;
            }
            let native_rate = rate.clamp(low, high);
            let format = range.sample_format();
            let preference = match format {
                SampleFormat::F32 => 0,
                SampleFormat::I32 => 1,
                SampleFormat::I16 => 2,
                _ => continue,
            };
            if usable(range, frames, format, native_rate) {
                choices.push((
                    (native_rate.abs_diff(rate), preference, range.channels()),
                    range,
                    native_rate,
                ));
            }
        }
        if let Some((_, range, native_rate)) = choices.into_iter().min_by_key(|c| c.0) {
            let mut config = (*range).with_sample_rate(native_rate).config();
            config.buffer_size = match range.buffer_size() {
                SupportedBufferSize::Range { .. } => cpal::BufferSize::Fixed(frames),
                SupportedBufferSize::Unknown => cpal::BufferSize::Default,
            };
            return Ok((range.sample_format(), config));
        }
        Err(format!(
            "{direction} needs f32/i32/i16 at 8000..192000 Hz and a supported period"
        ))
    };
    let (input_format, input_config) = select(input, "Input")?;
    let (output_format, output_config) = select(output, "Output")?;
    Ok((input_format, output_format, input_config, output_config))
}

#[cfg(test)]
fn negotiate(
    input: &[SupportedStreamConfigRange],
    output: &[SupportedStreamConfigRange],
    frames: u32,
) -> Result<(SampleFormat, SampleFormat, StreamConfig, StreamConfig)> {
    negotiate_at_rate(input, output, frames, RATE)
}
fn negotiate_routing_at_rate(
    input: &[SupportedStreamConfigRange],
    output: &[SupportedStreamConfigRange],
    frames: u32,
    routing: &ChannelRouting,
    rate: u32,
) -> Result<(SampleFormat, SampleFormat, StreamConfig, StreamConfig)> {
    routing.validate(
        input
            .iter()
            .map(|c| c.channels() as usize)
            .max()
            .unwrap_or(0),
        output
            .iter()
            .map(|c| c.channels() as usize)
            .max()
            .unwrap_or(0),
    )?;
    let input: Vec<_> = input
        .iter()
        .filter(|c| c.channels() as usize >= routing.input_channel)
        .cloned()
        .collect();
    let required_outputs = *routing
        .output_channels
        .iter()
        .max()
        .expect("validated routing");
    let output: Vec<_> = output
        .iter()
        .filter(|c| c.channels() as usize >= required_outputs)
        .cloned()
        .collect();
    negotiate_at_rate(&input, &output, frames, rate)
}

#[cfg(test)]
fn negotiate_routing(
    input: &[SupportedStreamConfigRange],
    output: &[SupportedStreamConfigRange],
    frames: u32,
    routing: &ChannelRouting,
) -> Result<(SampleFormat, SampleFormat, StreamConfig, StreamConfig)> {
    negotiate_routing_at_rate(input, output, frames, routing, RATE)
}
struct Prepared {
    configuration: crate::backend::AudioConfiguration,
    input: cpal::Device,
    output: cpal::Device,
    input_format: SampleFormat,
    output_format: SampleFormat,
    input_config: StreamConfig,
    output_config: StreamConfig,
    details: String,
}
impl CpalBackend {
    fn prepare(&self, input_name: &str, frames: u32, routing: &ChannelRouting) -> Result<Prepared> {
        let input = select_device(true, input_name)?;
        let output = select_device(false, &self.output_device)?;
        let input_ranges: Vec<_> = input.supported_input_configs()?.collect();
        let output_ranges: Vec<_> = output.supported_output_configs()?.collect();
        let (input_format, output_format, input_config, output_config) = negotiate_routing_at_rate(
            &input_ranges,
            &output_ranges,
            frames,
            routing,
            self.sample_rate,
        )?;
        let input_channel = routing.input_channel;
        let output_channels = &routing.output_channels;
        let rate = self.sample_rate;
        let details = format!("CPAL {:?}: input '{}' / {} channels / {:?}; output '{}' / {} channels / {:?}; {rate} Hz; input format {input_format:?}, output format {output_format:?}. Selected input {input_channel} -> outputs {output_channels:?}, other outputs silent. Default periods are chosen by the host and observed during playback.",
            cpal::default_host().id(), input.description()?.name(), input_config.channels, input_config.buffer_size,
            output.description()?.name(), output_config.channels, output_config.buffer_size);
        let details = format!("{details} Native input={} Hz, DSP={rate} Hz, native output={} Hz; resampling FIR delay={:.3} ms.", input_config.sample_rate, output_config.sample_rate,
            crate::dsp::resample::RateConverter::delay_ms(input_config.sample_rate, rate) + crate::dsp::resample::RateConverter::delay_ms(rate, output_config.sample_rate));
        let configuration = crate::backend::AudioConfiguration {
            device: input.description()?.name().to_string(),
            output_device: output.description()?.name().to_string(),
            sample_rate: self.sample_rate as f64,
            inputs: input_config.channels as usize,
            outputs: output_config.channels as usize,
        };
        Ok(Prepared {
            configuration,
            input,
            output,
            input_format,
            output_format,
            input_config,
            output_config,
            details,
        })
    }
}
fn supported_rates(
    input: &[SupportedStreamConfigRange],
    output: &[SupportedStreamConfigRange],
) -> crate::backend::DeviceSampleRates {
    let formats_at = |ranges: &[SupportedStreamConfigRange], rate: u32| -> Vec<SampleFormat> {
        ranges
            .iter()
            .filter(|c| {
                c.channels() > 0 && c.min_sample_rate() <= rate && c.max_sample_rate() >= rate
            })
            .map(|c| c.sample_format())
            .filter(|f| matches!(f, SampleFormat::F32 | SampleFormat::I32 | SampleFormat::I16))
            .collect()
    };
    let mut rates = crate::backend::DeviceSampleRates {
        channels: Vec::new(),
        input: Vec::new(),
        output: Vec::new(),
        common: Vec::new(),
    };
    for rate in crate::SAMPLE_RATES {
        let count = |ranges: &[SupportedStreamConfigRange]| {
            ranges
                .iter()
                .filter(|c| {
                    c.min_sample_rate() <= 192000
                        && c.max_sample_rate() >= 8000
                        && matches!(
                            c.sample_format(),
                            SampleFormat::F32 | SampleFormat::I32 | SampleFormat::I16
                        )
                })
                .map(|c| c.channels() as usize)
                .max()
                .unwrap_or(0)
        };
        let input_channels = count(input);
        let output_channels = count(output);
        if input_channels > 0 && output_channels > 0 {
            rates.channels.push(crate::backend::DeviceChannels {
                sample_rate: rate,
                input: input_channels,
                output: output_channels,
            });
        }
        let input = formats_at(input, rate);
        let output = formats_at(output, rate);
        if !input.is_empty() {
            rates.input.push(rate);
        }
        if !output.is_empty() {
            rates.output.push(rate);
        }
        if !input.is_empty() && !output.is_empty() {
            rates.common.push(rate);
        }
    }
    rates
}

impl AudioBackend for CpalBackend {
    fn supported_sample_rates(&self, name: &str) -> Result<crate::backend::DeviceSampleRates> {
        let input = select_device(true, name)?;
        let output = select_device(false, &self.output_device)?;
        let input_ranges: Vec<_> = input.supported_input_configs()?.collect();
        let output_ranges: Vec<_> = output.supported_output_configs()?.collect();
        Ok(supported_rates(&input_ranges, &output_ranges))
    }
    fn device_names(&self) -> Result<Vec<String>> {
        device_labels(true)
    }
    fn output_device_names(&self) -> Result<Vec<String>> {
        device_labels(false)
    }
    fn inspect_configuration(
        &self,
        name: &str,
        frames: u32,
        routing: &ChannelRouting,
    ) -> Result<crate::backend::AudioInspection> {
        let prepared = self.prepare(name, frames, routing)?;
        Ok(crate::backend::AudioInspection {
            configuration: prepared.configuration,
            details: prepared.details,
        })
    }
    fn open(
        &self,
        name: &str,
        frames: u32,
        routing: &ChannelRouting,
        dsp: DspSettings,
    ) -> Result<Box<dyn AudioStream>> {
        let prepared = self.prepare(name, frames, routing)?;
        match (prepared.input_format, prepared.output_format) {
            (SampleFormat::F32, SampleFormat::F32) => build::<f32, f32>(
                prepared,
                frames,
                routing,
                dsp,
                AudioProcessor::process_block,
            ),
            (SampleFormat::I32, SampleFormat::I32) => build::<i32, i32>(
                prepared,
                frames,
                routing,
                dsp,
                AudioProcessor::process_block,
            ),
            (SampleFormat::I16, SampleFormat::I16) => build::<i16, i16>(
                prepared,
                frames,
                routing,
                dsp,
                AudioProcessor::process_block,
            ),
            (SampleFormat::F32, _) => build_converted::<f32>(prepared, frames, routing, dsp),
            (SampleFormat::I32, _) => build_converted::<i32>(prepared, frames, routing, dsp),
            (SampleFormat::I16, _) => build_converted::<i16>(prepared, frames, routing, dsp),
            _ => Err("Unsupported negotiated input sample format".into()),
        }
    }
}

struct Metrics {
    sample_rate: u32,
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
    #[cfg(test)]
    fn new(prime_frames: usize) -> Self {
        Self::with_sample_rate(prime_frames, RATE)
    }
    fn with_sample_rate(prime_frames: usize, sample_rate: u32) -> Self {
        Self {
            sample_rate,
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

type BlockProcessor<I, O> =
    fn(&mut AudioProcessor<I>, &[I], &mut [O]) -> std::result::Result<u64, &'static str>;
type InputCallback<I> = Box<dyn FnMut(&[I]) + Send>;

struct Capture<I: Sample, O> {
    input_channel: usize,
    processor: AudioProcessor<I>,
    process_block: BlockProcessor<I, O>,
    mono: Vec<I>,
    processed: Vec<O>,
    channels: usize,
    queue: Producer<O>,
    metrics: Arc<Metrics>,
}
impl<I: Sample, O: Sample> Capture<I, O> {
    fn process(&mut self, data: &[I]) {
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
                *sample = frame[self.input_channel - 1];
            }
            let nonzero = (self.process_block)(
                &mut self.processor,
                &self.mono[..count],
                &mut self.processed[..count],
            );
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
        if elapsed > frames as u64 * 1_000_000_000 / self.metrics.sample_rate as u64 {
            self.metrics.deadline_misses.fetch_add(1, Ordering::Relaxed);
        }
    }
}

struct ResampledCapture<O> {
    input_channel: usize,
    channels: usize,
    input_rate: u32,
    input_converter: crate::dsp::resample::RateConverter,
    output_converter: crate::dsp::resample::RateConverter,
    processor: AudioProcessor<f64>,
    queue: Producer<O>,
    metrics: Arc<Metrics>,
}

impl<O: Sample> ResampledCapture<O> {
    fn process<I: Sample>(&mut self, data: &[I]) {
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
        let Self {
            input_converter,
            output_converter,
            processor,
            queue,
            metrics,
            ..
        } = self;
        for frame in data.chunks_exact(self.channels) {
            let sample = frame[self.input_channel - 1];
            metrics
                .nonzero
                .fetch_add(u64::from(sample != I::default()), Ordering::Relaxed);
            input_converter.push(sample.normalized(), |sample| {
                if metrics.faults.load(Ordering::Relaxed) != 0 {
                    return;
                }
                let mut output = [0.0];
                if processor.process_block(&[sample], &mut output).is_err() {
                    metrics.faults.fetch_or(INVALID_BUFFER, Ordering::Relaxed);
                    return;
                }
                #[cfg(windows)]
                crate::spectrum::push_output_samples(&output);
                output_converter.push(output[0], |s| {
                    if metrics.faults.load(Ordering::Relaxed) == 0
                        && !queue.push_block(&[O::from_normalized(s)])
                    {
                        metrics.overruns.fetch_add(1, Ordering::Relaxed);
                        metrics.faults.fetch_or(OVERRUN, Ordering::Relaxed);
                    }
                });
            });
            if metrics.faults.load(Ordering::Relaxed) != 0 {
                return;
            }
        }
        metrics.input_blocks.fetch_add(1, Ordering::Relaxed);
        metrics
            .last_input_ns
            .store(metrics.now_ns(), Ordering::Relaxed);
        let elapsed = start.elapsed().as_nanos().min(u64::MAX as u128) as u64;
        metrics.max_input_ns.fetch_max(elapsed, Ordering::Relaxed);
        if elapsed > frames as u64 * 1_000_000_000 / self.input_rate as u64 {
            metrics.deadline_misses.fetch_add(1, Ordering::Relaxed);
        }
    }
}

struct Playback<T> {
    output_channels: Vec<usize>,
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
            let sample = self.queue.pop().unwrap_or_default();
            for &channel in &self.output_channels {
                frame[channel - 1] = sample;
            }
        }
        let elapsed = start.elapsed().as_nanos().min(u64::MAX as u128) as u64;
        self.metrics
            .max_output_ns
            .fetch_max(elapsed, Ordering::Relaxed);
        if elapsed > frames as u64 * 1_000_000_000 / self.metrics.sample_rate as u64 {
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

fn build_converted<I: Sample + cpal::SizedSample + Send + 'static>(
    prepared: Prepared,
    frames: u32,
    routing: &ChannelRouting,
    dsp: DspSettings,
) -> Result<Box<dyn AudioStream>> {
    match prepared.output_format {
        SampleFormat::F32 => {
            build::<I, f32>(prepared, frames, routing, dsp, AudioProcessor::process_into)
        }
        SampleFormat::I32 => {
            build::<I, i32>(prepared, frames, routing, dsp, AudioProcessor::process_into)
        }
        SampleFormat::I16 => {
            build::<I, i16>(prepared, frames, routing, dsp, AudioProcessor::process_into)
        }
        _ => Err("Unsupported negotiated output sample format".into()),
    }
}

fn build<
    I: Sample + cpal::SizedSample + Send + 'static,
    O: Sample + cpal::SizedSample + Send + 'static,
>(
    prepared: Prepared,
    frames: u32,
    routing: &ChannelRouting,
    dsp: DspSettings,
    process_block: BlockProcessor<I, O>,
) -> Result<Box<dyn AudioStream>> {
    let pitch_latency = if dsp.voice_enabled {
        dsp.pitch.latency()
    } else {
        0
    };
    let capacity = (frames as usize * 8).max(32768).next_power_of_two();
    let (producer, consumer) = audio_queue::channel::<O>(capacity);
    let rate = prepared.configuration.sample_rate as u32;
    let metrics = Arc::new(Metrics::with_sample_rate(
        frames as usize * 2,
        prepared.output_config.sample_rate,
    ));
    let output_channels = routing.output_channels.clone();
    let mut capture: InputCallback<I> = if prepared.input_config.sample_rate != rate
        || prepared.output_config.sample_rate != rate
    {
        let mut capture = ResampledCapture::<O> {
            input_channel: routing.input_channel,
            channels: prepared.input_config.channels as usize,
            input_rate: prepared.input_config.sample_rate,
            input_converter: crate::dsp::resample::RateConverter::new(
                prepared.input_config.sample_rate,
                rate,
            )?,
            output_converter: crate::dsp::resample::RateConverter::new(
                rate,
                prepared.output_config.sample_rate,
            )?,
            processor: AudioProcessor::<f64>::new(rate, 1, dsp)?,
            queue: producer,
            metrics: metrics.clone(),
        };
        Box::new(move |data: &[I]| capture.process(data))
    } else {
        let mut capture = Capture {
            input_channel: routing.input_channel,
            process_block,
            processor: AudioProcessor::new(rate, frames as usize, dsp)?,
            mono: vec![I::default(); frames as usize],
            processed: vec![O::default(); frames as usize],
            channels: prepared.input_config.channels as usize,
            queue: producer,
            metrics: metrics.clone(),
        };
        Box::new(move |data: &[I]| capture.process(data))
    };
    let mut playback = Playback {
        output_channels,
        channels: prepared.output_config.channels as usize,
        queue: consumer,
        metrics: metrics.clone(),
        primed: false,
    };
    let errors = metrics.clone();
    let input = prepared.input.build_input_stream(
        prepared.input_config,
        move |data: &[I], _| capture(data),
        move |_| {
            errors.faults.fetch_or(DEVICE_ERROR, Ordering::Relaxed);
        },
        Some(Duration::from_secs(5)),
    )?;
    let errors = metrics.clone();
    let output = prepared.output.build_output_stream(
        prepared.output_config,
        move |data: &mut [O], _| playback.process(data),
        move |_| {
            errors.faults.fetch_or(DEVICE_ERROR, Ordering::Relaxed);
        },
        Some(Duration::from_secs(5)),
    )?;
    Ok(Box::new(CpalStream {
        pitch_latency,
        output: Some(output),
        input: Some(input),
        metrics,
        details: prepared.details,
        configuration: prepared.configuration,
        capacity,
        started: false,
        running: false,
        started_at: None,
    }))
}

struct CpalStream {
    pitch_latency: usize,
    configuration: crate::backend::AudioConfiguration,
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
    fn configuration(&self) -> crate::backend::AudioConfiguration {
        self.configuration.clone()
    }
    fn details(&self) -> String {
        self.details.clone()
    }
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
    fn telemetry(&self) -> crate::backend::AudioTelemetry {
        let metrics = &self.metrics;
        crate::backend::AudioTelemetry {
            blocks: self.blocks(),
            nonzero_samples: metrics.nonzero.load(Ordering::Relaxed),
            max_callback_us: metrics
                .max_input_ns
                .load(Ordering::Relaxed)
                .max(metrics.max_output_ns.load(Ordering::Relaxed))
                as f64
                / 1000.0,
            deadline_misses: metrics.deadline_misses.load(Ordering::Relaxed),
            fault_flags: metrics.faults.load(Ordering::Relaxed),
            output_blocks: Some(metrics.output_blocks.load(Ordering::Relaxed)),
            underruns: Some(metrics.underruns.load(Ordering::Relaxed)),
            overruns: Some(metrics.overruns.load(Ordering::Relaxed)),
            queue_frames: Some(metrics.queue_depth.load(Ordering::Relaxed)),
            observed_input_frames: Some(metrics.max_input_frames.load(Ordering::Relaxed)),
            observed_output_frames: Some(metrics.max_output_frames.load(Ordering::Relaxed)),
            queue_peak_frames: Some(metrics.queue_peak.load(Ordering::Relaxed)),
            startup_queue_frames: Some(metrics.prime_frames.load(Ordering::Relaxed)),
            queue_capacity_frames: Some(self.capacity),
            max_input_callback_us: Some(
                metrics.max_input_ns.load(Ordering::Relaxed) as f64 / 1000.0,
            ),
            max_output_callback_us: Some(
                metrics.max_output_ns.load(Ordering::Relaxed) as f64 / 1000.0,
            ),
            ..crate::backend::AudioTelemetry::default()
        }
    }
    fn latency_ms(&self) -> Result<(Option<f64>, Option<f64>)> {
        Ok((None, None))
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
        let latency = self.pitch_latency;
        Ok(format!("CPAL driver latency unavailable through this adapter; queue startup target={target} frames ({:.3} ms), queued snapshot={queued} frames ({:.3} ms), capacity={} frames; wet DSP={latency} samples ({:.3} ms), neutral DSP=0; rate-converter delay is listed in configuration details. Hardware round-trip latency is unmeasured. Separate device clock drift is not corrected.",
            target as f64 * 1000.0 / self.metrics.sample_rate as f64, queued as f64 * 1000.0 / self.metrics.sample_rate as f64, self.capacity,
            latency as f64 * 1000.0 / self.configuration.sample_rate))
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

    #[test]
    fn structured_log_retains_observed_periods_and_queue_state() {
        let (mut capture, mut playback, metrics) = callbacks::<f32>(
            64,
            1,
            &["--pitch", "0", "--gate", "off", "--auto-level", "off"],
        );
        capture.process(&[0.25; 64]);
        capture.process(&[0.25; 64]);
        playback.process(&mut [0.0; 48]);
        let stream = CpalStream {
            pitch_latency: crate::pitch::PitchAlgorithm::PhaseLocked.latency(),
            configuration: crate::backend::AudioConfiguration {
                device: "input".into(),
                output_device: "output".into(),
                sample_rate: RATE as f64,
                inputs: 1,
                outputs: 3,
            },
            input: None,
            output: None,
            metrics,
            details: String::new(),
            capacity: 512,
            started: false,
            running: false,
            started_at: None,
        };
        let log = stream.telemetry();
        assert_eq!(log.observed_input_frames, Some(64));
        assert_eq!(log.observed_output_frames, Some(16));
        assert_eq!(log.queue_peak_frames, Some(128));
        assert_eq!(log.startup_queue_frames, Some(128));
        assert_eq!(log.queue_capacity_frames, Some(512));
        assert_eq!(log.queue_frames, Some(112));
        assert_eq!(log.output_blocks, Some(1));
        assert!(log.max_input_callback_us.is_some());
        assert!(log.max_output_callback_us.is_some());
    }

    fn callbacks<T: Sample + Send>(
        max_frames: usize,
        channels: usize,
        args: &[&str],
    ) -> (Capture<T, T>, Playback<T>, Arc<Metrics>) {
        callbacks_with_processor(max_frames, channels, args, AudioProcessor::process_block)
    }

    fn callbacks_with_processor<I: Sample + Send, O: Sample + Send>(
        max_frames: usize,
        channels: usize,
        args: &[&str],
        process_block: BlockProcessor<I, O>,
    ) -> (Capture<I, O>, Playback<O>, Arc<Metrics>) {
        let options = Options::parse(args.iter().map(|s| s.to_string())).unwrap();
        let (producer, consumer) =
            audio_queue::channel((max_frames * 8).max(32).next_power_of_two());
        let metrics = Arc::new(Metrics::new(max_frames * 2));
        metrics.active.store(true, Ordering::Release);
        (
            Capture {
                input_channel: options.input_channel,
                process_block,
                processor: AudioProcessor::new(RATE, max_frames, DspSettings::from(&options))
                    .unwrap(),
                mono: vec![I::default(); max_frames],
                processed: vec![O::default(); max_frames],
                channels,
                queue: producer,
                metrics: metrics.clone(),
            },
            Playback {
                output_channels: options.output_channels.clone(),
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
    fn selected_input_is_duplicated_to_selected_outputs_in_all_formats() {
        fn verify<T: Sample + Send + std::fmt::Debug>(samples: &[T]) -> Vec<T> {
            let (mut capture, mut playback, metrics) =
                callbacks::<T>(2, 2, &["--input-channel", "2", "--output-channels", "1,3"]);
            let interleaved: Vec<_> = samples.iter().flat_map(|&s| [T::default(), s]).collect();
            capture.process(&interleaved);
            let mut output = vec![T::default(); samples.len() * 3];
            for block in output.chunks_mut(6) {
                playback.process(block);
            }
            for (frame, &sample) in output.as_chunks::<3>().0.iter().zip(samples) {
                assert_eq!(*frame, [sample, T::default(), sample]);
            }
            assert_eq!(metrics.faults.load(Ordering::Relaxed), 0);
            output
        }
        verify(&[i16::MIN, -1, 0, i16::MAX]);
        verify(&[i32::MIN, -1, 0, i32::MAX]);
        let samples = [-0.0_f32, f32::from_bits(1), -0.25, 1.0];
        let output = verify(&samples);
        for (frame, sample) in output.as_chunks::<3>().0.iter().zip(samples) {
            assert_eq!(frame[0].to_bits(), sample.to_bits());
            assert_eq!(frame[2].to_bits(), sample.to_bits());
        }
        let (mut capture, mut playback, metrics) =
            callbacks::<i16>(2, 2, &["--output-channels", "2"]);
        capture.process(&[7, 99, -8, 99, 9, 99, -10, 99]);
        let mut output = [99; 12];
        for block in output.chunks_mut(6) {
            playback.process(block);
        }
        assert_eq!(output, [0, 7, 0, 0, -8, 0, 0, 9, 0, 0, -10, 0]);
        assert_eq!(metrics.faults.load(Ordering::Relaxed), 0);
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
    fn channel_layouts_allow_independent_formats() {
        let buffer = SupportedBufferSize::Unknown;
        let input = [
            range(SampleFormat::F32, 8, 48000, buffer),
            range(SampleFormat::I16, 2, 48000, buffer),
        ];
        let output = [range(SampleFormat::I16, 8, 48000, buffer)];
        let rates = supported_rates(&input, &output);
        assert_eq!(
            rates.channels,
            crate::SAMPLE_RATES
                .map(|sample_rate| crate::backend::DeviceChannels {
                    sample_rate,
                    input: 8,
                    output: 8
                })
                .to_vec()
        );
        let output = [
            range(SampleFormat::F32, 2, 48000, buffer),
            range(SampleFormat::I16, 8, 48000, buffer),
        ];
        let rates = supported_rates(&input, &output);
        assert_eq!(rates.channels.len(), 2);
        for layout in rates.channels {
            for input_channel in 1..=layout.input {
                for output_channel in 1..=layout.output {
                    let routing = ChannelRouting {
                        input_channel,
                        output_channels: vec![output_channel],
                    };
                    assert!(negotiate_routing_at_rate(
                        &input,
                        &output,
                        128,
                        &routing,
                        layout.sample_rate
                    )
                    .is_ok());
                }
            }
        }
        assert!(negotiate_routing_at_rate(
            &input,
            &output,
            128,
            &ChannelRouting {
                input_channel: 8,
                output_channels: vec![8]
            },
            48000
        )
        .is_ok());
    }

    #[test]
    fn channel_choices_follow_rate_and_supported_formats() {
        let buffer = SupportedBufferSize::Unknown;
        let input = [
            range(SampleFormat::F32, 8, 44100, buffer),
            range(SampleFormat::I16, 2, 48000, buffer),
            range(SampleFormat::U16, 16, 48000, buffer),
        ];
        let output = [range(SampleFormat::I16, 4, 48000, buffer)];
        let rates = supported_rates(&input, &output);
        assert_eq!(
            rates.channels,
            crate::SAMPLE_RATES
                .map(|sample_rate| crate::backend::DeviceChannels {
                    sample_rate,
                    input: 8,
                    output: 4
                })
                .to_vec()
        );
        let empty = supported_rates(&[], &[]);
        assert!(empty.channels.is_empty());
    }

    #[test]
    fn device_rates_require_a_common_rate_with_independent_formats() {
        let buffer = SupportedBufferSize::Unknown;
        let input = [
            SupportedStreamConfigRange::new(1, 44100, 96000, buffer, SampleFormat::F32),
            range(SampleFormat::I16, 1, 44100, buffer),
            range(SampleFormat::U16, 1, 176400, buffer),
        ];
        let output = [
            range(SampleFormat::F32, 2, 48000, buffer),
            range(SampleFormat::I32, 2, 44100, buffer),
            range(SampleFormat::I16, 2, 192000, buffer),
        ];
        let rates = supported_rates(&input, &output);
        assert_eq!(rates.input, [44100, 48000]);
        assert_eq!(rates.output, [44100, 48000]);
        assert_eq!(rates.common, [44100, 48000]);
        let empty = supported_rates(&[], &output);
        assert!(empty.input.is_empty() && empty.common.is_empty());
        assert_eq!(empty.output, rates.output);
        assert!(
            supported_rates(&[range(SampleFormat::F32, 0, 48000, buffer)], &output)
                .common
                .is_empty()
        );
    }

    #[test]
    fn negotiates_native_rates_independently_when_conversion_is_needed() {
        let buffer = SupportedBufferSize::Range { min: 64, max: 512 };
        for rate in crate::SAMPLE_RATES {
            let input = [range(SampleFormat::F32, 2, rate, buffer)];
            let output = [range(SampleFormat::F32, 2, rate, buffer)];
            let (_, _, input_config, output_config) =
                negotiate_at_rate(&input, &output, 128, rate).unwrap();
            assert_eq!(input_config.sample_rate, rate);
            assert_eq!(output_config.sample_rate, rate);
            let other = if rate == 48000 { 44100 } else { 48000 };
            assert_eq!(
                negotiate_at_rate(&input, &output, 128, other)
                    .unwrap()
                    .2
                    .sample_rate,
                rate
            );
            assert!(negotiate_at_rate(
                &input,
                &[range(SampleFormat::F32, 2, other, buffer)],
                128,
                rate
            )
            .is_ok());
        }
        let broad = [SupportedStreamConfigRange::new(
            2,
            44100,
            192000,
            buffer,
            SampleFormat::F32,
        )];
        assert_eq!(
            negotiate_at_rate(&broad, &broad, 64, 44100)
                .unwrap()
                .2
                .sample_rate,
            44100
        );
        assert!(negotiate_at_rate(&broad, &broad, 64, 0).is_err());
    }

    #[test]
    fn inspection_and_open_negotiate_the_requested_channel_layout() {
        let input = [
            range(SampleFormat::F32, 1, RATE, SupportedBufferSize::Unknown),
            range(SampleFormat::F32, 2, RATE, SupportedBufferSize::Unknown),
        ];
        let output = [
            range(SampleFormat::F32, 1, RATE, SupportedBufferSize::Unknown),
            range(SampleFormat::F32, 3, RATE, SupportedBufferSize::Unknown),
        ];
        let routing = ChannelRouting {
            input_channel: 2,
            output_channels: vec![1, 3],
        };
        let (_, _, input_config, output_config) =
            negotiate_routing(&input, &output, 128, &routing).unwrap();
        assert_eq!(input_config.channels, 2);
        assert_eq!(output_config.channels, 3);
        assert!(negotiate_routing(&input[..1], &output, 128, &routing).is_err());
        assert!(negotiate_routing(&input, &output[..1], 128, &routing).is_err());
        let mono = ChannelRouting {
            input_channel: 1,
            output_channels: vec![1],
        };
        let (_, _, input_config, output_config) =
            negotiate_routing(&input, &output, 128, &mono).unwrap();
        assert_eq!(input_config.channels, 1);
        assert_eq!(output_config.channels, 1);
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
        let (input_format, output_format, input_config, output_config) =
            negotiate(&input, &output, 128).unwrap();
        assert_eq!(input_format, SampleFormat::F32);
        assert_eq!(output_format, SampleFormat::I16);
        assert_eq!(input_config.channels, 2);
        assert_eq!(input_config.buffer_size, cpal::BufferSize::Fixed(128));
        assert_eq!(output_config.buffer_size, cpal::BufferSize::Default);
        assert!(negotiate(&input, &output, 32).is_err());
        assert!(negotiate(&input, &output, 0).is_err());
        assert_eq!(
            negotiate(&input, &[range(SampleFormat::I16, 2, 44100, known)], 128)
                .unwrap()
                .3
                .sample_rate,
            44100
        );
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
    fn every_format_pair_matches_f64_dsp_through_channel_mapping_without_allocations() {
        fn check<I: Sample + Send, O: Sample + Send + std::fmt::Debug>() {
            for wet in [false, true] {
                let mut args = vec!["--input-channel", "2", "--output-channels", "1,3"];
                if wet {
                    args.extend(["--pitch", "3", "--eq", "on", "--eq-band2", "3000,3,0.8"]);
                }
                let source: Vec<I> = (0..512)
                    .map(|i| {
                        I::from_normalized(
                            0.3 * (i as f64 * 0.03).sin() + 0.1 * (i as f64 * 0.2).cos(),
                        )
                    })
                    .collect();
                let input: Vec<I> = source
                    .iter()
                    .flat_map(|s| [I::from_normalized(0.9), *s])
                    .collect();
                let normalized: Vec<f64> = source.iter().map(|s| s.normalized()).collect();
                let mut reference = AudioProcessor::<f64>::new(
                    RATE,
                    512,
                    DspSettings::from(&Options::parse(args.iter().map(|s| s.to_string())).unwrap()),
                )
                .unwrap();
                let mut expected = vec![0.0; 512];
                reference.process_block(&normalized, &mut expected).unwrap();
                let (mut capture, mut playback, metrics) =
                    callbacks_with_processor::<I, O>(64, 2, &args, AudioProcessor::process_into);
                let mut actual = [O::default(); 512 * 3];
                ALLOCATIONS.with(|n| n.set(0));
                COUNTING.with(|c| c.set(true));
                // Host blocks cross both DSP chunk and interleaved channel boundaries.
                for chunk in input.chunks(2 * 73) {
                    capture.process(chunk);
                }
                let mut offset = 0;
                for frames in [17, 64, 31].into_iter().cycle() {
                    let end = (offset + frames * 3).min(actual.len());
                    playback.process(&mut actual[offset..end]);
                    offset = end;
                    if offset == actual.len() {
                        break;
                    }
                }
                COUNTING.with(|c| c.set(false));
                assert_eq!(ALLOCATIONS.with(Cell::get), 0);
                assert_eq!(metrics.faults.load(Ordering::Relaxed), 0);
                assert_eq!(
                    metrics.nonzero.load(Ordering::Relaxed),
                    source.iter().filter(|s| **s != I::default()).count() as u64
                );
                for (frame, expected) in actual.as_chunks::<3>().0.iter().zip(expected) {
                    let expected = O::from_normalized(expected);
                    assert_eq!(*frame, [expected, O::default(), expected]);
                }
            }
        }
        check::<f32, f32>();
        check::<f32, i32>();
        check::<f32, i16>();
        check::<i32, f32>();
        check::<i32, i32>();
        check::<i32, i16>();
        check::<i16, f32>();
        check::<i16, i32>();
        check::<i16, i16>();
    }

    #[test]
    fn negotiation_selects_input_and_output_formats_independently() {
        let formats = [SampleFormat::F32, SampleFormat::I32, SampleFormat::I16];
        for input_format in formats {
            for output_format in formats {
                let input = [range(input_format, 2, RATE, SupportedBufferSize::Unknown)];
                let output = [range(output_format, 4, RATE, SupportedBufferSize::Unknown)];
                let (selected_input, selected_output, input_config, output_config) =
                    negotiate(&input, &output, 64).unwrap();
                assert_eq!(
                    (selected_input, selected_output),
                    (input_format, output_format)
                );
                assert_eq!((input_config.channels, output_config.channels), (2, 4));
                assert_eq!(supported_rates(&input, &output).common, [RATE]);
            }
        }
    }

    #[test]
    fn resampled_neutral_voice_preserves_frequency_and_declared_delay() {
        let options = Options::parse(["--pitch", "0"].map(String::from)).unwrap();
        let (queue, mut output) = audio_queue::channel::<f32>(16384);
        let metrics = Arc::new(Metrics::with_sample_rate(128, 44100));
        metrics.active.store(true, Ordering::Release);
        let mut capture = ResampledCapture {
            input_channel: 2,
            channels: 2,
            input_rate: 96000,
            input_converter: crate::dsp::resample::RateConverter::new(96000, 48000).unwrap(),
            output_converter: crate::dsp::resample::RateConverter::new(48000, 44100).unwrap(),
            processor: AudioProcessor::new(48000, 1, DspSettings::from(&options)).unwrap(),
            queue,
            metrics: metrics.clone(),
        };
        let input: Vec<f32> = (0..9600)
            .flat_map(|i| {
                [
                    0.9,
                    (0.25 * (std::f64::consts::TAU * 1000.0 * i as f64 / 96000.0).sin()) as f32,
                ]
            })
            .collect();
        for chunk in input.chunks(254) {
            capture.process(chunk);
        }
        let delay = (crate::dsp::resample::RateConverter::delay_ms(96000, 48000)
            + crate::dsp::resample::RateConverter::delay_ms(48000, 44100))
            / 1000.0;
        let mut index = 0;
        while let Some(sample) = output.pop() {
            if index > 1024 {
                let expected = 0.25
                    * (std::f64::consts::TAU * 1000.0 * (index as f64 / 44100.0 - delay)).sin();
                assert!((sample as f64 - expected).abs() < 0.001);
            }
            index += 1;
        }
        assert_eq!(index, 4410);
        assert_eq!(metrics.faults.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn resampled_capture_maps_every_format_pair_without_callback_allocations() {
        fn check<I: Sample, O: Sample + Send>() {
            for pitch in ["0", "3", "12"] {
                let options = Options::parse(["--pitch", pitch].map(String::from)).unwrap();
                let (queue, mut output) = audio_queue::channel::<O>(16384);
                let metrics = Arc::new(Metrics::with_sample_rate(128, 44100));
                metrics.active.store(true, Ordering::Release);
                let mut capture = ResampledCapture {
                    input_channel: 2,
                    channels: 2,
                    input_rate: 96000,
                    input_converter: crate::dsp::resample::RateConverter::new(96000, 48000)
                        .unwrap(),
                    output_converter: crate::dsp::resample::RateConverter::new(48000, 44100)
                        .unwrap(),
                    processor: AudioProcessor::new(48000, 1, DspSettings::from(&options)).unwrap(),
                    queue,
                    metrics: metrics.clone(),
                };
                // An unselected nonzero channel must never leak into the mono path.
                let input = [I::from_normalized(0.8), I::default()].repeat(9600);
                ALLOCATIONS.with(|n| n.set(0));
                COUNTING.with(|c| c.set(true));
                let start = Instant::now();
                for chunk in input.chunks(146) {
                    capture.process(chunk);
                }
                let elapsed = start.elapsed();
                COUNTING.with(|c| c.set(false));
                assert_eq!(ALLOCATIONS.with(Cell::get), 0);
                assert_eq!(metrics.faults.load(Ordering::Relaxed), 0);
                assert_eq!(metrics.nonzero.load(Ordering::Relaxed), 0);
                assert_eq!(output.len(), 4410);
                while let Some(sample) = output.pop() {
                    assert_eq!(sample.normalized(), 0.0);
                }
                eprintln!(
                    "resampling {:?}->{:?}, pitch {pitch}: {elapsed:?}/100 ms",
                    std::any::type_name::<I>(),
                    std::any::type_name::<O>()
                );
            }
        }
        check::<f32, f32>();
        check::<f32, i32>();
        check::<f32, i16>();
        check::<i32, f32>();
        check::<i32, i32>();
        check::<i32, i16>();
        check::<i16, f32>();
        check::<i16, i32>();
        check::<i16, i16>();
    }

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
