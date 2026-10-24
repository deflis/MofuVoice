//! Direct ASIO duplex processing via asio-sys public Driver APIs.
use crate::{audio::DspChain, dsp::Sample, RATE};
use asio_sys::{
    AsioBufferInfo, AsioDriverEvent, AsioMessageSelectors, AsioSampleType, AsioStreams,
    BufferCallbackId, BufferPreference, BufferSizeRange, CallbackInfo, Driver,
    DriverEventCallbackId,
};
use std::{
    cell::UnsafeCell,
    error::Error,
    ptr,
    sync::{
        atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering},
        Arc, Mutex,
    },
    time::Instant,
};

type Result<T> = std::result::Result<T, Box<dyn Error>>;
const RESET: u32 = 1;
const RESYNC: u32 = 2;
const RATE_CHANGE: u32 = 4;
const INVALID_BUFFER: u32 = 8;
const OVERLOAD: u32 = 16;
const REENTRY: u32 = 32;

#[derive(Clone, Copy, Debug, PartialEq)]
enum Format {
    F32,
    I32,
    I16,
}
impl Format {
    fn from_asio(value: AsioSampleType) -> Result<Self> {
        match value {
            AsioSampleType::ASIOSTFloat32LSB => Ok(Self::F32),
            AsioSampleType::ASIOSTInt32LSB => Ok(Self::I32),
            AsioSampleType::ASIOSTInt16LSB => Ok(Self::I16),
            other => Err(format!(
                "Unsupported ASIO format {other:?}; expected little-endian f32/i32/i16"
            )
            .into()),
        }
    }
    fn bytes(self) -> usize {
        if self == Self::I16 {
            2
        } else {
            4
        }
    }
}

pub struct Configuration {
    inputs: usize,
    outputs: usize,
    format: Format,
    rate: f64,
    min: i32,
    max: i32,
    preferred: BufferPreference,
}
impl Configuration {
    pub fn query(driver: &Driver) -> Result<Self> {
        let channels = driver.channels()?;
        if channels.ins < 1 || channels.outs < 1 || channels.ins > 512 || channels.outs > 512 {
            return Err("ASIO requires 1..512 input and output channels".into());
        }
        let input = Format::from_asio(driver.input_data_type()?)?;
        let output = Format::from_asio(driver.output_data_type()?)?;
        if input != output {
            return Err("All selected input/output channels must have the same format".into());
        }
        let BufferSizeRange {
            min,
            max,
            preferred,
        } = driver.buffersize_range()?;
        Ok(Self {
            inputs: channels.ins as usize,
            outputs: channels.outs as usize,
            format: input,
            rate: driver.sample_rate()?,
            min,
            max,
            preferred,
        })
    }
    pub fn audio_configuration(&self) -> crate::backend::AudioConfiguration {
        crate::backend::AudioConfiguration {
            device: String::new(),
            output_device: String::new(),
            sample_rate: self.rate,
            inputs: self.inputs,
            outputs: self.outputs,
        }
    }
    pub fn print(&self) {
        println!("{}", self.summary());
    }
    pub fn summary(&self) -> String {
        let preferred = match self.preferred {
            BufferPreference::Only(size) | BufferPreference::Preferred(size) => size,
            BufferPreference::Stepped { preferred, .. } => preferred,
        };
        let granularity = match self.preferred {
            // asio-sys maps ASIO granularity -1 (power-of-two) to Only.
            BufferPreference::Only(_) => -1,
            // asio-sys maps ASIO granularity 0 (preferred-only) to Preferred.
            BufferPreference::Preferred(_) => 0,
            BufferPreference::Stepped { step, .. } => step as i32,
        };
        format!("Input/output: {}/{} channels, {:?}, {} Hz; buffer min/max/preferred={}/{}/{}, granularity={}", self.inputs, self.outputs, self.format, self.rate, self.min, self.max, preferred, granularity)
    }
    pub fn validate(&self, frames: u32) -> Result<()> {
        self.validate_rate(frames, RATE)
    }
    pub fn validate_rate(&self, frames: u32, rate: u32) -> Result<()> {
        crate::validate_sample_rate(rate)?;
        if self.rate != rate as f64 {
            return Err(format!(
                "Driver currently runs at {} Hz; set the ASIO driver to {rate} Hz to match the selected sample rate",
                self.rate
            )
            .into());
        }
        if !buffer_supported(frames, self.min, self.max, self.preferred) {
            let preferred = match self.preferred {
                BufferPreference::Only(size) | BufferPreference::Preferred(size) => size,
                BufferPreference::Stepped { preferred, .. } => preferred,
            };
            let granularity = match self.preferred {
                BufferPreference::Only(_) => -1,
                BufferPreference::Preferred(_) => 0,
                BufferPreference::Stepped { step, .. } => step as i32,
            };
            return Err(format!("Requested {frames} frames not supported by ASIO min/max/preferred={}/{}/{}, granularity={}; specify the current size explicitly", self.min, self.max, preferred, granularity).into());
        }
        Ok(())
    }
}

fn buffer_supported(frames: u32, min: i32, max: i32, preferred: BufferPreference) -> bool {
    if min < 1 || max < min || frames < min as u32 || frames > max as u32 {
        return false;
    }
    match preferred {
        // asio-sys maps ASIO granularity -1 (power-of-two) to Only.
        BufferPreference::Only(_) => frames.is_power_of_two(),
        // asio-sys maps ASIO granularity 0 (preferred-only) to Preferred.
        BufferPreference::Preferred(size) => frames == size,
        BufferPreference::Stepped { step, .. } => {
            step > 0 && (frames - min as u32).is_multiple_of(step)
        }
    }
}

#[derive(Default)]
struct Metrics {
    blocks: AtomicU64,
    timestamp_blocks: AtomicU64,
    nonzero: AtomicU64,
    max_ns: AtomicU64,
    late: AtomicU64,
    gaps: AtomicU64,
    faults: AtomicU32,
    latencies_changed: AtomicBool,
}
struct CallbackState {
    sample_rate: u32,
    input_channel: usize,
    output_channels: Vec<usize>,
    frames: usize,
    format: Format,
    active: AtomicBool,
    processing: AtomicBool,
    metrics: Metrics,
    dsp: UnsafeCell<DspChain>,
    scratch: UnsafeCell<Vec<f32>>,
}
// SAFETY: ASIO delivers callbacks on one thread; the control thread only touches DSP after stop.
unsafe impl Send for CallbackState {}
unsafe impl Sync for CallbackState {}

impl CallbackState {
    unsafe fn process<T: Sample>(
        &self,
        input_infos: &[AsioBufferInfo],
        output_infos: &[AsioBufferInfo],
        index: usize,
    ) -> u64 {
        // SAFETY: open validates distinct, aligned buffers of frames*T bytes. ASIO owns them
        // until dispose; the callback's processing guard excludes simultaneous buffer writers.
        let input = unsafe {
            std::slice::from_raw_parts(
                input_infos[self.input_channel - 1].buffers[index].cast::<T>(),
                self.frames,
            )
        };
        let mut nonzero = 0;
        for (channel, info) in output_infos.iter().enumerate() {
            let output = unsafe {
                std::slice::from_raw_parts_mut(info.buffers[index].cast::<T>(), self.frames)
            };
            if channel == self.output_channels[0] - 1 {
                // SAFETY: only the callback holding the processing guard accesses DSP.
                // The control thread never reads or mutates it during streaming.
                let dsp = unsafe { &mut *self.dsp.get() };
                // Scratch storage is aligned for f32/i32 and large enough for i16.
                let scratch = unsafe {
                    let storage = &mut *self.scratch.get();
                    debug_assert!(
                        std::mem::size_of_val(storage.as_slice())
                            >= self.frames * std::mem::size_of::<T>()
                    );
                    std::slice::from_raw_parts_mut(storage.as_mut_ptr().cast::<T>(), self.frames)
                };
                nonzero = dsp.process_block(input, output, scratch);
                crate::spectrum::push_output_samples(output);
            } else if self.output_channels.contains(&(channel + 1)) {
                let source = unsafe {
                    std::slice::from_raw_parts(
                        output_infos[self.output_channels[0] - 1].buffers[index].cast::<T>(),
                        self.frames,
                    )
                };
                output.copy_from_slice(source);
            } else {
                output.fill(T::default());
            }
        }
        nonzero
    }
    fn validate_pointers(
        &self,
        input: &asio_sys::AsioStream,
        output: &asio_sys::AsioStream,
    ) -> Result<()> {
        let bytes = self
            .frames
            .checked_mul(self.format.bytes())
            .ok_or("ASIO buffer size overflow")?;
        let mut ranges = Vec::new();
        for info in input.buffer_infos.iter().chain(output.buffer_infos.iter()) {
            for buffer in info.buffers {
                let start = buffer as usize;
                if start == 0 || !start.is_multiple_of(self.format.bytes()) {
                    return Err("ASIO returned null or misaligned buffers".into());
                }
                let end = start
                    .checked_add(bytes)
                    .ok_or("ASIO buffer address overflow")?;
                if ranges.iter().any(|&(a, b)| start < b && a < end) {
                    return Err("ASIO returned overlapping input/output buffers".into());
                }
                ranges.push((start, end));
            }
        }
        Ok(())
    }
}

fn process_callback(state: &CallbackState, streams: &AsioStreams, info: &CallbackInfo) {
    if !state.active.load(Ordering::Acquire) {
        return;
    }
    let index = info.buffer_index;
    if !(0..=1).contains(&index) {
        state
            .metrics
            .faults
            .fetch_or(INVALID_BUFFER, Ordering::Relaxed);
        return;
    }
    if state.processing.swap(true, Ordering::Acquire) {
        state.metrics.faults.fetch_or(REENTRY, Ordering::Relaxed);
        return;
    }
    let start = Instant::now();
    state
        .metrics
        .timestamp_blocks
        .fetch_add(1, Ordering::Relaxed);
    let (Some(input), Some(output)) = (&streams.input, &streams.output) else {
        state.processing.store(false, Ordering::Release);
        return;
    };
    if state.metrics.faults.load(Ordering::Relaxed) == 0 {
        // SAFETY: index checked, buffers immutable since preparation, exclusive callback writer.
        let nonzero = unsafe {
            match state.format {
                Format::F32 => {
                    state.process::<f32>(&input.buffer_infos, &output.buffer_infos, index as usize)
                }
                Format::I32 => {
                    state.process::<i32>(&input.buffer_infos, &output.buffer_infos, index as usize)
                }
                Format::I16 => {
                    state.process::<i16>(&input.buffer_infos, &output.buffer_infos, index as usize)
                }
            }
        };
        state.metrics.nonzero.fetch_add(nonzero, Ordering::Relaxed);
    } else {
        // A reset/resync request invalidates normal processing. Silence until control-thread stop.
        for info in &output.buffer_infos {
            unsafe {
                ptr::write_bytes(
                    info.buffers[index as usize],
                    0,
                    state.frames * state.format.bytes(),
                );
            }
        }
    }
    let ns = start.elapsed().as_nanos().min(u64::MAX as u128) as u64;
    state.metrics.max_ns.fetch_max(ns, Ordering::Relaxed);
    if ns > state.frames as u64 * 1_000_000_000 / state.sample_rate as u64 {
        state.metrics.late.fetch_add(1, Ordering::Relaxed);
    }
    state.metrics.blocks.fetch_add(1, Ordering::Relaxed);
    state.processing.store(false, Ordering::Release);
}

/// Query only; never change the driver's clock or allocate audio buffers.
/// The caller keeps the initialized driver alive and excludes an active stream.
pub(crate) fn supported_sample_rates(driver: &Driver) -> Result<Vec<u32>> {
    let mut rates = Vec::new();
    for rate in crate::SAMPLE_RATES {
        if driver.can_sample_rate(rate as f64)? {
            rates.push(rate);
        }
    }
    Ok(rates)
}

pub use crate::audio::DspSettings;
pub struct Engine {
    pitch_latency: usize,
    configuration: crate::backend::AudioConfiguration,
    details: String,
    driver: Driver,
    state: Arc<CallbackState>,
    streams: Arc<Mutex<AsioStreams>>,
    callback_id: Option<BufferCallbackId>,
    event_id: Option<DriverEventCallbackId>,
    prepared: bool,
    running: bool,
}
impl Engine {
    pub fn set_device_name(&mut self, name: &str) {
        self.configuration.device = name.to_string();
        self.configuration.output_device = name.to_string();
    }
    pub fn open(
        driver: Driver,
        config: Configuration,
        frames: u32,
        routing: &crate::backend::ChannelRouting,
        dsp: DspSettings,
    ) -> Result<Self> {
        Self::open_with_sample_rate(driver, config, frames, routing, dsp, RATE)
    }
    pub fn open_with_sample_rate(
        driver: Driver,
        config: Configuration,
        frames: u32,
        routing: &crate::backend::ChannelRouting,
        dsp: DspSettings,
        rate: u32,
    ) -> Result<Self> {
        config.validate_rate(frames, rate)?;
        routing.validate(config.inputs, config.outputs)?;
        let input_channel = routing.input_channel;
        let mut output_channels = routing.output_channels.clone();
        output_channels.sort_unstable();
        let pitch_latency = if dsp.voice_enabled {
            dsp.pitch.latency()
        } else {
            0
        };
        let dsp = DspChain::with_sample_rate(dsp, rate)?;
        let state = Arc::new(CallbackState {
            sample_rate: rate,
            input_channel,
            output_channels,
            frames: frames as usize,
            format: config.format,
            active: AtomicBool::new(false),
            processing: AtomicBool::new(false),
            metrics: Metrics::default(),
            dsp: UnsafeCell::new(dsp),
            scratch: UnsafeCell::new(vec![0.0; frames as usize]),
        });
        let streams_slot = driver.streams();
        let prepared = (|| -> Result<()> {
            let mut held = streams_slot
                .lock()
                .map_err(|_| "ASIO stream lock poisoned")?;
            let prepared = driver.prepare_input_stream(
                held.output.take(),
                config.inputs,
                Some(frames as i32),
            )?;
            let prepared = driver.prepare_output_stream(
                prepared.input,
                config.outputs,
                Some(frames as i32),
            )?;
            *held = prepared;
            let (Some(input), Some(output)) = (&held.input, &held.output) else {
                return Err("ASIO failed to prepare duplex buffers".into());
            };
            state.validate_pointers(input, output)?;
            for info in &output.buffer_infos {
                for buffer in info.buffers {
                    unsafe {
                        ptr::write_bytes(buffer, 0, frames as usize * config.format.bytes());
                    }
                }
            }
            if driver.sample_rate()? != rate as f64 {
                return Err("Driver rate changed during buffer preparation".into());
            }
            Ok(())
        })();
        if let Err(error) = prepared {
            let _ = driver.dispose_buffers();
            if let Ok(mut streams) = streams_slot.lock() {
                *streams = AsioStreams {
                    input: None,
                    output: None,
                };
            }
            return Err(error);
        }
        let callback_state = Arc::clone(&state);
        let callback_streams = Arc::clone(&streams_slot);
        let callback_id = driver.add_callback(move |info| {
            let Ok(streams) = callback_streams.lock() else {
                return;
            };
            process_callback(&callback_state, &streams, info);
        });
        let event_state = Arc::clone(&state);
        let expected_rate = rate as f64;
        let event_id = driver.add_event_callback(move |event| match event {
            AsioDriverEvent::Message { selector, value } => match selector {
                AsioMessageSelectors::kAsioSelectorSupported => {
                    value == AsioMessageSelectors::kAsioOverload as i32
                }
                AsioMessageSelectors::kAsioResetRequest => {
                    event_state
                        .metrics
                        .faults
                        .fetch_or(RESET, Ordering::Relaxed);
                    true
                }
                AsioMessageSelectors::kAsioResyncRequest => {
                    event_state
                        .metrics
                        .faults
                        .fetch_or(RESYNC, Ordering::Relaxed);
                    true
                }
                AsioMessageSelectors::kAsioLatenciesChanged => {
                    event_state
                        .metrics
                        .latencies_changed
                        .store(true, Ordering::Relaxed);
                    true
                }
                AsioMessageSelectors::kAsioOverload => {
                    event_state
                        .metrics
                        .faults
                        .fetch_or(OVERLOAD, Ordering::Relaxed);
                    true
                }
                _ => false,
            },
            AsioDriverEvent::SampleRateChanged(new_rate) => {
                if (new_rate - expected_rate).abs() >= 1.0 {
                    event_state
                        .metrics
                        .faults
                        .fetch_or(RATE_CHANGE, Ordering::Relaxed);
                }
                false
            }
        });
        Ok(Self {
            pitch_latency,
            configuration: config.audio_configuration(),
            details: config.summary(),
            driver,
            state,
            streams: streams_slot,
            callback_id: Some(callback_id),
            event_id: Some(event_id),
            prepared: true,
            running: false,
        })
    }
    pub fn start(&mut self) -> Result<()> {
        self.state.active.store(true, Ordering::Release);
        if let Err(error) = self.driver.start() {
            self.state.active.store(false, Ordering::Release);
            return Err(error.into());
        }
        self.running = true;
        Ok(())
    }
    pub fn stop(&mut self) -> Result<()> {
        self.state.active.store(false, Ordering::Release);
        if self.running {
            self.driver.stop()?;
            self.running = false;
        }
        Ok(())
    }
    pub fn blocks(&self) -> u64 {
        self.state.metrics.blocks.load(Ordering::Relaxed)
    }
    pub fn report(&self) {
        println!("{}", self.summary());
    }
    pub fn summary(&self) -> String {
        let metrics = &self.state.metrics;
        let latency = self.pitch_latency;
        format!("duplex blocks={}; frames={}; wet DSP={latency} samples ({:.2} ms), neutral bypass=0; nonzero input samples={}; max callback={:.3} us; deadline misses={}; timestamp blocks={}; sample-position gaps={}; fault flags=0x{:x}", self.blocks(), self.state.frames, latency as f64 * 1000.0 / self.configuration.sample_rate, metrics.nonzero.load(Ordering::Relaxed), metrics.max_ns.load(Ordering::Relaxed) as f64 / 1000.0, metrics.late.load(Ordering::Relaxed), metrics.timestamp_blocks.load(Ordering::Relaxed), metrics.gaps.load(Ordering::Relaxed), metrics.faults.load(Ordering::Relaxed))
    }
    pub fn print_latencies(&self) -> Result<()> {
        println!("{}", self.latency_summary()?);
        Ok(())
    }
    pub fn latency_summary(&self) -> Result<String> {
        let latencies = self.driver.latencies()?;
        Ok(format!("Driver-reported ASIO latency: input={} samples ({:.3} ms), output={} samples ({:.3} ms). Not a microphone round-trip measurement.", latencies.input, latencies.input as f64 * 1000.0 / self.configuration.sample_rate, latencies.output, latencies.output as f64 * 1000.0 / self.configuration.sample_rate))
    }
    pub fn check_events(&self) -> Result<()> {
        let faults = self.state.metrics.faults.load(Ordering::Relaxed);
        if faults != 0 {
            return Err(format!("ASIO requests restart or reports invalid processing (flags 0x{faults:x}: reset=1,resync=2,rate=4,buffer=8,overload=16,reentry=32,clock=64)").into());
        }
        if self
            .state
            .metrics
            .latencies_changed
            .swap(false, Ordering::Relaxed)
        {
            self.print_latencies()?;
        }
        Ok(())
    }
    pub fn check_test_result(&self) -> Result<()> {
        if self.blocks() == 0 {
            return Err("No full-duplex ASIO callbacks observed".into());
        }
        if self.state.metrics.late.load(Ordering::Relaxed) != 0
            || self.state.metrics.gaps.load(Ordering::Relaxed) != 0
        {
            return Err("ASIO test had deadline misses or sample-position gaps".into());
        }
        Ok(())
    }
    fn detach_callbacks(&mut self) {
        if let Some(id) = self.callback_id.take() {
            self.driver.remove_callback(id);
        }
        if let Some(id) = self.event_id.take() {
            self.driver.remove_event_callback(id);
        }
    }
}
impl crate::backend::AudioStream for Engine {
    fn configuration(&self) -> crate::backend::AudioConfiguration {
        self.configuration.clone()
    }
    fn details(&self) -> String {
        self.details.clone()
    }
    fn start(&mut self) -> Result<()> {
        Engine::start(self)
    }
    fn stop(&mut self) -> Result<()> {
        Engine::stop(self)
    }
    fn blocks(&self) -> u64 {
        Engine::blocks(self)
    }
    fn telemetry(&self) -> crate::backend::AudioTelemetry {
        let metrics = &self.state.metrics;
        crate::backend::AudioTelemetry {
            blocks: self.blocks(),
            nonzero_samples: metrics.nonzero.load(Ordering::Relaxed),
            max_callback_us: metrics.max_ns.load(Ordering::Relaxed) as f64 / 1000.0,
            deadline_misses: metrics.late.load(Ordering::Relaxed),
            timestamp_blocks: Some(metrics.timestamp_blocks.load(Ordering::Relaxed)),
            sample_position_gaps: Some(metrics.gaps.load(Ordering::Relaxed)),
            fault_flags: metrics.faults.load(Ordering::Relaxed),
            ..crate::backend::AudioTelemetry::default()
        }
    }
    fn latency_ms(&self) -> Result<(Option<f64>, Option<f64>)> {
        let latencies = self.driver.latencies()?;
        Ok((
            Some(latencies.input as f64 * 1000.0 / self.configuration.sample_rate),
            Some(latencies.output as f64 * 1000.0 / self.configuration.sample_rate),
        ))
    }
    fn summary(&self) -> String {
        Engine::summary(self)
    }
    fn latency_summary(&self) -> Result<String> {
        Engine::latency_summary(self)
    }
    fn check_events(&self) -> Result<()> {
        Engine::check_events(self)
    }
    fn check_test_result(&self) -> Result<()> {
        Engine::check_test_result(self)
    }
}

impl Drop for Engine {
    fn drop(&mut self) {
        if let Err(error) = self.stop() {
            eprintln!("ASIO stop during cleanup: {error}");
        }
        self.detach_callbacks();
        if self.prepared {
            if let Err(error) = self.driver.dispose_buffers() {
                eprintln!("ASIO cleanup: {error}");
            }
            if let Ok(mut streams) = self.streams.lock() {
                *streams = AsioStreams {
                    input: None,
                    output: None,
                };
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{gate::GateSettings, pitch::PitchShifter};
    #[test]
    fn validates_selected_rate_without_reinterpreting_driver_samples() {
        for rate in crate::SAMPLE_RATES {
            let config = Configuration {
                inputs: 1,
                outputs: 1,
                format: Format::F32,
                rate: rate as f64,
                min: 64,
                max: 512,
                preferred: BufferPreference::Only(512),
            };
            assert!(config.validate_rate(128, rate).is_ok());
            let other = if rate == RATE { 44100 } else { RATE };
            assert!(config.validate_rate(128, other).is_err());
        }
    }
    #[test]
    fn validates_fixed_power_of_two_and_stepped_buffers() {
        assert!(buffer_supported(
            512,
            512,
            512,
            BufferPreference::Preferred(512)
        ));
        assert!(!buffer_supported(
            64,
            512,
            512,
            BufferPreference::Preferred(512)
        ));
        assert!(buffer_supported(64, 32, 1024, BufferPreference::Only(512)));
        assert!(!buffer_supported(96, 32, 1024, BufferPreference::Only(512)));
        assert!(buffer_supported(
            96,
            32,
            1024,
            BufferPreference::Stepped {
                preferred: 512,
                step: 32
            }
        ));
        assert!(!buffer_supported(
            100,
            32,
            1024,
            BufferPreference::Stepped {
                preferred: 512,
                step: 32
            }
        ));
    }
    #[test]
    fn direct_buffer_copy_has_no_fifo_and_silences_other_outputs() {
        let mut input_a = [1.0_f32, -0.0, 0.25, -0.5];
        let mut input_b = [0.1_f32; 4];
        let mut output_a = [99.0_f32; 4];
        let mut output_b = [99.0_f32; 4];
        let mut other_a = [99.0_f32; 4];
        let mut other_b = [99.0_f32; 4];
        let info = |a: *mut f32, b: *mut f32| AsioBufferInfo {
            is_input: 0,
            channel_num: 0,
            buffers: [a.cast(), b.cast()],
        };
        let input_infos = [info(input_a.as_mut_ptr(), input_b.as_mut_ptr())];
        let mut output_infos = [
            info(output_a.as_mut_ptr(), output_b.as_mut_ptr()),
            info(other_a.as_mut_ptr(), other_b.as_mut_ptr()),
        ];
        let state = CallbackState {
            sample_rate: RATE,
            input_channel: 1,
            output_channels: vec![1],
            frames: 4,
            format: Format::F32,
            active: AtomicBool::new(false),
            processing: AtomicBool::new(false),
            metrics: Metrics::default(),
            dsp: UnsafeCell::new(
                DspChain::new(DspSettings::from(
                    &crate::cli::Options::parse(Vec::<String>::new()).unwrap(),
                ))
                .unwrap(),
            ),
            scratch: UnsafeCell::new(vec![0.0; 4]),
        };
        let input_stream = asio_sys::AsioStream {
            buffer_infos: input_infos.to_vec(),
            buffer_size: 4,
        };
        let output_stream = asio_sys::AsioStream {
            buffer_infos: output_infos.to_vec(),
            buffer_size: 4,
        };
        state
            .validate_pointers(&input_stream, &output_stream)
            .unwrap();
        assert_eq!(
            unsafe { state.process::<f32>(&input_infos, &output_infos, 0) },
            3
        );
        assert_eq!(input_a.map(f32::to_bits), output_a.map(f32::to_bits));
        assert_eq!(other_a, [0.0; 4]);
        assert_eq!(output_b, [99.0; 4]);
        assert_eq!(
            unsafe { state.process::<f32>(&input_infos, &output_infos, 1) },
            4
        );
        assert_eq!(input_b, output_b);
        assert_eq!(other_b, [0.0; 4]);
        // Select input 2, fan out once-processed samples, and silence deselected outputs.
        let mut selected_input_a = [-0.0_f32, f32::from_bits(1), -0.25, 0.5];
        let mut selected_input_b = [0.1_f32, 0.2, -0.3, 0.4];
        let input_infos = [
            info(input_a.as_mut_ptr(), input_b.as_mut_ptr()),
            info(selected_input_a.as_mut_ptr(), selected_input_b.as_mut_ptr()),
        ];
        let mut state = CallbackState {
            sample_rate: RATE,
            input_channel: 2,
            output_channels: vec![1, 2],
            frames: 4,
            format: Format::F32,
            active: AtomicBool::new(false),
            processing: AtomicBool::new(false),
            metrics: Metrics::default(),
            dsp: UnsafeCell::new(
                DspChain::new(DspSettings::from(
                    &crate::cli::Options::parse(Vec::<String>::new()).unwrap(),
                ))
                .unwrap(),
            ),
            scratch: UnsafeCell::new(vec![0.0; 4]),
        };
        assert_eq!(
            unsafe { state.process::<f32>(&input_infos, &output_infos, 0) },
            3
        );
        assert_eq!(
            output_a.map(f32::to_bits),
            selected_input_a.map(f32::to_bits)
        );
        assert_eq!(
            other_a.map(f32::to_bits),
            selected_input_a.map(f32::to_bits)
        );
        state.output_channels = vec![2];
        assert_eq!(
            unsafe { state.process::<f32>(&input_infos, &output_infos, 1) },
            4
        );
        assert_eq!(output_b, [0.0; 4]);
        assert_eq!(other_b, selected_input_b);
        state.input_channel = 1;
        state.output_channels = vec![1];
        state.dsp.get_mut().pitch = PitchShifter::new(
            0.0,
            true,
            0.0,
            crate::pitch::PitchAlgorithm::PhaseLocked,
            crate::pitch::SpectralWindows::default(),
            Default::default(),
        )
        .unwrap();
        state.dsp.get_mut().gate = GateSettings {
            enabled: true,
            threshold: crate::gate::GateThreshold::Db(-90.0),
            attack_ms: 0.1,
            release_ms: 80.0,
        }
        .build()
        .unwrap();
        state.dsp.get_mut().level = crate::level::LevelSettings {
            enabled: true,
            compressor_enabled: true,
            ..Default::default()
        }
        .build()
        .unwrap();
        let input_infos = [info(input_a.as_mut_ptr(), input_b.as_mut_ptr())];
        unsafe { state.process::<f32>(&input_infos, &output_infos, 0) };
        assert!(output_a[0] > 0.0 && output_a[0] <= 0.892);
        assert_eq!(output_a[1], 0.0);
        assert_eq!(other_a, [0.0; 4]);
        output_infos[0].buffers[0] = input_infos[0].buffers[0];
        let bad_output = asio_sys::AsioStream {
            buffer_infos: output_infos.to_vec(),
            buffer_size: 4,
        };
        let good_input = asio_sys::AsioStream {
            buffer_infos: input_infos.to_vec(),
            buffer_size: 4,
        };
        assert!(state.validate_pointers(&good_input, &bad_output).is_err());

        // Exercise the wet route through the raw ASIO descriptors, including ring wraps.
        output_infos[0].buffers[0] = output_a.as_mut_ptr().cast();
        state.dsp.get_mut().gate = GateSettings {
            enabled: false,
            threshold: crate::gate::GateThreshold::Auto,
            attack_ms: 5.0,
            release_ms: 80.0,
        }
        .build()
        .unwrap();
        state.dsp.get_mut().level = crate::level::LevelSettings::default().build().unwrap();
        state.dsp.get_mut().pitch = PitchShifter::new(
            3.0,
            true,
            0.0,
            crate::pitch::PitchAlgorithm::PhaseLocked,
            crate::pitch::SpectralWindows::default(),
            Default::default(),
        )
        .unwrap();
        for _ in 0..1024 {
            assert_eq!(
                unsafe { state.process::<f32>(&input_infos, &output_infos, 0) },
                3
            );
        }
        assert_ne!(input_a.map(f32::to_bits), output_a.map(f32::to_bits));
        assert!(output_a.iter().all(|s| s.is_finite() && s.abs() <= 1.0));
        assert_eq!(other_a, [0.0; 4]);
        assert_eq!(output_b, [0.0; 4]); // Last index 1 routing muted this output; index 0 leaves it untouched.
        for _ in 0..4096 {
            assert_eq!(
                unsafe { state.process::<f32>(&input_infos, &output_infos, 1) },
                4
            );
        }
        assert!(output_b.iter().all(|s| (*s - 0.1).abs() < 0.001));
        assert_eq!(other_b, [0.0; 4]);
    }
}
