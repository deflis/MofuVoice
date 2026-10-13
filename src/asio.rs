//! Direct ASIO buffer callbacks. asio-sys is used only to load/unload the driver.
use crate::{
    audio::DspChain,
    pitch::{Sample, LATENCY},
    RATE,
};
use asio_sys::{bindings::asio_import as ffi, Driver};
use std::{
    cell::UnsafeCell,
    error::Error,
    ffi::c_void,
    ptr,
    sync::atomic::{AtomicBool, AtomicPtr, AtomicU32, AtomicU64, AtomicUsize, Ordering},
    thread,
    time::{Duration, Instant},
};

type Result<T> = std::result::Result<T, Box<dyn Error>>;
const OK: i32 = 0;
const SUCCESS: i32 = 0x3f4847a0;
const RESET: u32 = 1;
const RESYNC: u32 = 2;
const RATE_CHANGE: u32 = 4;
const INVALID_BUFFER: u32 = 8;
const OVERLOAD: u32 = 16;
const REENTRY: u32 = 32;
const CLOCK_CHANGE: u32 = 64;

fn check(code: i32, operation: &str) -> Result<()> {
    if code == OK || code == SUCCESS {
        Ok(())
    } else {
        Err(format!("{operation} failed: ASIO error {code}").into())
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Format {
    F32,
    I32,
    I16,
}
impl Format {
    fn from_asio(value: i32) -> Result<Self> {
        match value {
            19 => Ok(Self::F32),
            18 => Ok(Self::I32),
            16 => Ok(Self::I16),
            _ => Err(format!(
                "Unsupported ASIO format {value}; expected little-endian f32/i32/i16"
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
    preferred: i32,
    granularity: i32,
}
impl Configuration {
    pub fn query(driver: &Driver) -> Result<Self> {
        let channels = driver.channels()?;
        if channels.ins < 1 || channels.outs < 1 || channels.ins > 512 || channels.outs > 512 {
            return Err("ASIO requires 1..512 input and output channels".into());
        }
        let mut format = None;
        for input in [true, false] {
            let count = if input { channels.ins } else { channels.outs };
            for channel in 0..count {
                // Zero is a valid initial representation for the SDK channel-info POD.
                let mut info: ffi::ASIOChannelInfo = unsafe { std::mem::zeroed() };
                info.channel = channel;
                info.isInput = i32::from(input);
                // SAFETY: driver is initialized; info is writable and lives through the call.
                check(
                    unsafe { ffi::ASIOGetChannelInfo(&mut info) },
                    "ASIOGetChannelInfo",
                )?;
                let current = Format::from_asio(info.type_)?;
                if format.is_some_and(|previous| previous != current) {
                    return Err(
                        "All selected input/output channels must have the same format".into(),
                    );
                }
                format = Some(current);
            }
        }
        let (mut min, mut max, mut preferred, mut granularity) = (0, 0, 0, 0);
        // SAFETY: initialized driver and four valid output pointers.
        check(
            unsafe { ffi::ASIOGetBufferSize(&mut min, &mut max, &mut preferred, &mut granularity) },
            "ASIOGetBufferSize",
        )?;
        Ok(Self {
            inputs: channels.ins as usize,
            outputs: channels.outs as usize,
            format: format.expect("channels validated"),
            rate: driver.sample_rate()?,
            min,
            max,
            preferred,
            granularity,
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
        format!("Input/output: {}/{} channels, {:?}, {} Hz; buffer min/max/preferred={}/{}/{}, granularity={}", self.inputs, self.outputs, self.format, self.rate, self.min, self.max, self.preferred, self.granularity)
    }
    pub fn validate(&self, frames: u32) -> Result<()> {
        if self.rate != RATE as f64 {
            return Err(format!(
                "Driver currently runs at {} Hz; configure the audio engine for 48000 Hz",
                self.rate
            )
            .into());
        }
        if !buffer_supported(frames, self.min, self.max, self.preferred, self.granularity) {
            return Err(format!("Requested {frames} frames not supported by ASIO min/max/preferred={}/{}/{}, granularity={}; specify the current size explicitly", self.min, self.max, self.preferred, self.granularity).into());
        }
        Ok(())
    }
}
fn buffer_supported(frames: u32, min: i32, max: i32, preferred: i32, granularity: i32) -> bool {
    if min < 1 || max < min || frames < min as u32 || frames > max as u32 {
        return false;
    }
    match granularity {
        -1 => frames.is_power_of_two(),
        0 => frames == preferred as u32,
        step if step > 0 => (frames - min as u32).is_multiple_of(step as u32),
        _ => false,
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
    input_channel: usize,
    output_channels: Vec<usize>,
    buffers: Vec<ffi::ASIOBufferInfo>,
    inputs: usize,
    frames: usize,
    format: Format,
    active: AtomicBool,
    processing: AtomicBool,
    output_ready: AtomicBool,
    last_position: AtomicU64,
    metrics: Metrics,
    dsp: UnsafeCell<DspChain>,
    scratch: UnsafeCell<Vec<f32>>,
}
impl CallbackState {
    unsafe fn process<T: Sample>(&self, index: usize) -> u64 {
        // SAFETY: open validates distinct, aligned buffers of frames*T bytes. ASIO owns them
        // until dispose; the callback's processing guard excludes simultaneous buffer writers.
        let input = unsafe {
            std::slice::from_raw_parts(
                self.buffers[self.input_channel - 1].buffers[index].cast::<T>(),
                self.frames,
            )
        };
        let mut nonzero = 0;
        for (channel, info) in self.buffers[self.inputs..].iter().enumerate() {
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
                        self.buffers[self.inputs + self.output_channels[0] - 1].buffers[index]
                            .cast::<T>(),
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
    fn validate_pointers(&self) -> Result<()> {
        let bytes = self
            .frames
            .checked_mul(self.format.bytes())
            .ok_or("ASIO buffer size overflow")?;
        let mut ranges = Vec::new();
        for info in &self.buffers {
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

// ASIO has one process-global driver. State is installed before buffer creation and removed
// before disposal. Every FFI callback holds a lease; teardown waits for all leases to finish.
static STATE: AtomicPtr<CallbackState> = AtomicPtr::new(ptr::null_mut());
static IN_FLIGHT: AtomicUsize = AtomicUsize::new(0);
struct Lease(*mut CallbackState);
impl Lease {
    fn acquire() -> Self {
        IN_FLIGHT.fetch_add(1, Ordering::SeqCst);
        Self(STATE.load(Ordering::SeqCst))
    }
    fn state(&self) -> Option<&CallbackState> {
        // SAFETY: the teardown wait prevents freeing the Box while this lease exists.
        unsafe { self.0.as_ref() }
    }
}
impl Drop for Lease {
    fn drop(&mut self) {
        IN_FLIGHT.fetch_sub(1, Ordering::SeqCst);
    }
}

fn process_callback(index: i32, position: Option<u64>) {
    let lease = Lease::acquire();
    let Some(state) = lease.state() else {
        return;
    };
    if !state.active.load(Ordering::Acquire) {
        return;
    }
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
    if let Some(position) = position {
        state
            .metrics
            .timestamp_blocks
            .fetch_add(1, Ordering::Relaxed);
        let previous = state.last_position.swap(position, Ordering::Relaxed);
        if previous != u64::MAX && previous.checked_add(state.frames as u64) != Some(position) {
            state.metrics.gaps.fetch_add(1, Ordering::Relaxed);
        }
    }
    if state.metrics.faults.load(Ordering::Relaxed) == 0 {
        // SAFETY: index checked, buffers immutable since preparation, exclusive callback writer.
        let nonzero = unsafe {
            match state.format {
                Format::F32 => state.process::<f32>(index as usize),
                Format::I32 => state.process::<i32>(index as usize),
                Format::I16 => state.process::<i16>(index as usize),
            }
        };
        state.metrics.nonzero.fetch_add(nonzero, Ordering::Relaxed);
    } else {
        // A reset/resync request invalidates normal processing. Silence until control-thread stop.
        for output in &state.buffers[state.inputs..] {
            unsafe {
                ptr::write_bytes(
                    output.buffers[index as usize],
                    0,
                    state.frames * state.format.bytes(),
                );
            }
        }
    }
    if state.output_ready.load(Ordering::Relaxed) {
        unsafe {
            ffi::ASIOOutputReady();
        }
    }
    let ns = start.elapsed().as_nanos().min(u64::MAX as u128) as u64;
    state.metrics.max_ns.fetch_max(ns, Ordering::Relaxed);
    if ns > state.frames as u64 * 1_000_000_000 / RATE as u64 {
        state.metrics.late.fetch_add(1, Ordering::Relaxed);
    }
    state.metrics.blocks.fetch_add(1, Ordering::Relaxed);
    state.processing.store(false, Ordering::Release);
}
extern "C" fn buffer_switch(index: i32, _: i32) {
    process_callback(index, None);
}
extern "C" fn rate_changed(rate: f64) {
    let lease = Lease::acquire();
    if let Some(state) = lease.state() {
        if rate != RATE as f64 {
            state
                .metrics
                .faults
                .fetch_or(RATE_CHANGE, Ordering::Relaxed);
        }
    }
}
extern "C" fn message(selector: i32, value: i32, _: *mut c_void, _: *mut f64) -> i32 {
    if selector == 1 {
        return i32::from(matches!(value, 2..=7 | 15));
    }
    if selector == 2 {
        return 2;
    }
    let lease = Lease::acquire();
    let Some(state) = lease.state() else {
        return 0;
    };
    match selector {
        3 | 4 => {
            state.metrics.faults.fetch_or(RESET, Ordering::Relaxed);
            1
        }
        5 => {
            state.metrics.faults.fetch_or(RESYNC, Ordering::Relaxed);
            1
        }
        6 => {
            state
                .metrics
                .latencies_changed
                .store(true, Ordering::Relaxed);
            1
        }
        7 => 1,
        15 => {
            state.metrics.faults.fetch_or(OVERLOAD, Ordering::Relaxed);
            1
        }
        _ => 0,
    }
}
extern "C" fn buffer_switch_time(
    time: *mut ffi::ASIOTime,
    index: i32,
    _: i32,
) -> *mut ffi::ASIOTime {
    let mut position = None;
    if !time.is_null() {
        // SAFETY: SDK-owned ASIOTime follows the packed(4) layout in asio-sys::AsioTime.
        // Use unaligned reads: bindgen's opaque ASIOSampleRate is deliberately not interpreted.
        let timing = unsafe {
            ptr::read_unaligned(ptr::addr_of!(
                (*(time.cast::<asio_sys::AsioTime>())).time_info
            ))
        };
        if timing.flags & 2 != 0 {
            position =
                Some((timing.sample_position.hi as u64) << 32 | timing.sample_position.lo as u64);
        }
        let lease = Lease::acquire();
        if let Some(state) = lease.state() {
            if timing.flags & 4 != 0 && timing.sample_rate != RATE as f64 {
                state
                    .metrics
                    .faults
                    .fetch_or(RATE_CHANGE, Ordering::Relaxed);
            }
            if timing.flags & 32 != 0 {
                state
                    .metrics
                    .faults
                    .fetch_or(CLOCK_CHANGE, Ordering::Relaxed);
            }
        }
    }
    process_callback(index, position);
    time
}

// SDK Windows native ASIOSampleRate is f64. Bindgen's callback parameter uses an opaque
// byte array, so define the native callback ABI explicitly, as CPAL's backend does.
#[repr(C, packed(4))]
struct RawCallbacks {
    buffer_switch: extern "C" fn(i32, i32),
    rate_changed: extern "C" fn(f64),
    message: extern "C" fn(i32, i32, *mut c_void, *mut f64) -> i32,
    time: extern "C" fn(*mut ffi::ASIOTime, i32, i32) -> *mut ffi::ASIOTime,
}
static CALLBACKS: RawCallbacks = RawCallbacks {
    buffer_switch,
    rate_changed,
    message,
    time: buffer_switch_time,
};
// asio-sys does not allowlist ASIOGetLatencies; bind the MSVC C++ mangled export directly.
unsafe extern "C" {
    #[link_name = "\x01?ASIOGetLatencies@@YAJPEAJ0@Z"]
    fn asio_get_latencies(input: *mut i32, output: *mut i32) -> i32;
}

pub use crate::audio::DspSettings;
pub struct Engine {
    configuration: crate::backend::AudioConfiguration,
    details: String,
    driver: Driver,
    state: Box<CallbackState>,
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
        dsp: DspSettings,
    ) -> Result<Self> {
        config.validate(frames)?;
        if dsp.input_channel == 0
            || dsp.input_channel > config.inputs
            || dsp.output_channels.is_empty()
            || dsp
                .output_channels
                .iter()
                .any(|&c| c == 0 || c > config.outputs)
        {
            return Err("Selected channels are unavailable on this ASIO device".into());
        }
        let input_channel = dsp.input_channel;
        let mut output_channels = dsp.output_channels.clone();
        output_channels.sort_unstable();
        let dsp = DspChain::new(dsp)?;
        let mut buffers = Vec::with_capacity(config.inputs + config.outputs);
        for (input, channels) in [(true, config.inputs), (false, config.outputs)] {
            for channel in 0..channels {
                buffers.push(ffi::ASIOBufferInfo {
                    isInput: i32::from(input),
                    channelNum: channel as i32,
                    buffers: [ptr::null_mut(); 2],
                });
            }
        }
        let state = Box::new(CallbackState {
            input_channel,
            output_channels,
            buffers,
            inputs: config.inputs,
            frames: frames as usize,
            format: config.format,
            active: AtomicBool::new(false),
            processing: AtomicBool::new(false),
            output_ready: AtomicBool::new(false),
            last_position: AtomicU64::new(u64::MAX),
            metrics: Metrics::default(),
            dsp: UnsafeCell::new(dsp),
            scratch: UnsafeCell::new(vec![0.0; frames as usize]),
        });
        let mut engine = Self {
            configuration: config.audio_configuration(),
            details: config.summary(),
            driver,
            state,
            prepared: false,
            running: false,
        };
        let pointer = ptr::from_ref(engine.state.as_ref()).cast_mut();
        if STATE
            .compare_exchange(ptr::null_mut(), pointer, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return Err("Another ASIO engine is already active".into());
        }
        // SAFETY: stable descriptor Vec and static callback table live until stop/dispose.
        check(
            unsafe {
                ffi::ASIOCreateBuffers(
                    engine.state.buffers.as_mut_ptr(),
                    engine.state.buffers.len() as i32,
                    frames as i32,
                    ptr::from_ref(&CALLBACKS)
                        .cast_mut()
                        .cast::<ffi::ASIOCallbacks>(),
                )
            },
            "ASIOCreateBuffers",
        )?;
        engine.prepared = true;
        engine.state.validate_pointers()?;
        for info in &engine.state.buffers[config.inputs..] {
            for buffer in info.buffers {
                unsafe {
                    ptr::write_bytes(buffer, 0, frames as usize * config.format.bytes());
                }
            }
        }
        engine.state.output_ready.store(
            matches!(unsafe { ffi::ASIOOutputReady() }, OK | SUCCESS),
            Ordering::Relaxed,
        );
        if engine.driver.sample_rate()? != RATE as f64 {
            return Err("Driver rate changed during buffer preparation".into());
        }
        Ok(engine)
    }
    pub fn start(&mut self) -> Result<()> {
        self.state.active.store(true, Ordering::Release);
        let code = unsafe { ffi::ASIOStart() };
        if code != OK && code != SUCCESS {
            self.state.active.store(false, Ordering::Release);
            check(code, "ASIOStart")?;
        }
        self.running = true;
        Ok(())
    }
    pub fn stop(&mut self) -> Result<()> {
        self.state.active.store(false, Ordering::Release);
        if self.running {
            check(unsafe { ffi::ASIOStop() }, "ASIOStop")?;
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
        format!("duplex blocks={}; frames={}; spectral wet DSP={LATENCY} samples ({:.2} ms), neutral bypass=0; nonzero input samples={}; max callback={:.3} us; deadline misses={}; timestamp blocks={}; sample-position gaps={}; fault flags=0x{:x}", self.blocks(), self.state.frames, LATENCY as f64 * 1000.0 / RATE as f64, metrics.nonzero.load(Ordering::Relaxed), metrics.max_ns.load(Ordering::Relaxed) as f64 / 1000.0, metrics.late.load(Ordering::Relaxed), metrics.timestamp_blocks.load(Ordering::Relaxed), metrics.gaps.load(Ordering::Relaxed), metrics.faults.load(Ordering::Relaxed))
    }
    pub fn print_latencies(&self) -> Result<()> {
        println!("{}", self.latency_summary()?);
        Ok(())
    }
    pub fn latency_summary(&self) -> Result<String> {
        let (mut input, mut output) = (0, 0);
        check(
            unsafe { asio_get_latencies(&mut input, &mut output) },
            "ASIOGetLatencies",
        )?;
        Ok(format!("Driver-reported ASIO latency: input={input} samples ({:.3} ms), output={output} samples ({:.3} ms). Not a microphone round-trip measurement.", input as f64 * 1000.0 / RATE as f64, output as f64 * 1000.0 / RATE as f64))
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
        let (mut input, mut output) = (0, 0);
        check(
            unsafe { asio_get_latencies(&mut input, &mut output) },
            "ASIOGetLatencies",
        )?;
        Ok((
            Some(input as f64 * 1000.0 / RATE as f64),
            Some(output as f64 * 1000.0 / RATE as f64),
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
        let own_pointer = ptr::from_ref(self.state.as_ref()).cast_mut();
        // A failed second open must never detach another engine's state.
        if STATE.load(Ordering::SeqCst) != own_pointer {
            return;
        }
        if let Err(error) = self.stop() {
            eprintln!("ASIO stop during cleanup: {error}");
        }
        STATE.store(ptr::null_mut(), Ordering::SeqCst);
        while IN_FLIGHT.load(Ordering::SeqCst) != 0 {
            thread::sleep(Duration::from_millis(1));
        }
        // No callback can now read state or buffers. The static callback table stays valid.
        if self.prepared {
            if let Err(error) = check(unsafe { ffi::ASIODisposeBuffers() }, "ASIODisposeBuffers") {
                eprintln!("ASIO cleanup: {error}");
            }
        }
        // Driver's own state remains Initialized: it only handles ASIOExit/unloading afterward.
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{gate::GateSettings, pitch::PitchShifter};
    #[test]
    fn validates_fixed_power_of_two_and_stepped_buffers() {
        assert!(buffer_supported(512, 512, 512, 512, 0));
        assert!(!buffer_supported(64, 512, 512, 512, 0));
        assert!(buffer_supported(64, 32, 1024, 512, -1));
        assert!(!buffer_supported(96, 32, 1024, 512, -1));
        assert!(buffer_supported(96, 32, 1024, 512, 32));
        assert!(!buffer_supported(100, 32, 1024, 512, 32));
    }
    #[test]
    fn direct_buffer_copy_has_no_fifo_and_silences_other_outputs() {
        let mut input_a = [1.0_f32, -0.0, 0.25, -0.5];
        let mut input_b = [0.1_f32; 4];
        let mut output_a = [99.0_f32; 4];
        let mut output_b = [99.0_f32; 4];
        let mut other_a = [99.0_f32; 4];
        let mut other_b = [99.0_f32; 4];
        let info = |a: *mut f32, b: *mut f32| ffi::ASIOBufferInfo {
            isInput: 0,
            channelNum: 0,
            buffers: [a.cast(), b.cast()],
        };
        let mut state = CallbackState {
            input_channel: 1,
            output_channels: vec![1],
            buffers: vec![
                info(input_a.as_mut_ptr(), input_b.as_mut_ptr()),
                info(output_a.as_mut_ptr(), output_b.as_mut_ptr()),
                info(other_a.as_mut_ptr(), other_b.as_mut_ptr()),
            ],
            inputs: 1,
            frames: 4,
            format: Format::F32,
            active: AtomicBool::new(false),
            processing: AtomicBool::new(false),
            output_ready: AtomicBool::new(false),
            last_position: AtomicU64::new(u64::MAX),
            metrics: Metrics::default(),
            dsp: UnsafeCell::new(
                DspChain::new(DspSettings::from(
                    &crate::cli::Options::parse(Vec::<String>::new()).unwrap(),
                ))
                .unwrap(),
            ),
            scratch: UnsafeCell::new(vec![0.0; 4]),
        };
        state.validate_pointers().unwrap();
        assert_eq!(unsafe { state.process::<f32>(0) }, 3);
        assert_eq!(input_a.map(f32::to_bits), output_a.map(f32::to_bits));
        assert_eq!(other_a, [0.0; 4]);
        assert_eq!(output_b, [99.0; 4]);
        assert_eq!(unsafe { state.process::<f32>(1) }, 4);
        assert_eq!(input_b, output_b);
        assert_eq!(other_b, [0.0; 4]);
        // Select input 2, fan out once-processed samples, and silence deselected outputs.
        let mut selected_input_a = [-0.0_f32, f32::from_bits(1), -0.25, 0.5];
        let mut selected_input_b = [0.1_f32, 0.2, -0.3, 0.4];
        state.buffers.insert(
            1,
            info(selected_input_a.as_mut_ptr(), selected_input_b.as_mut_ptr()),
        );
        state.inputs = 2;
        state.input_channel = 2;
        state.output_channels = vec![1, 2];
        assert_eq!(unsafe { state.process::<f32>(0) }, 3);
        assert_eq!(
            output_a.map(f32::to_bits),
            selected_input_a.map(f32::to_bits)
        );
        assert_eq!(
            other_a.map(f32::to_bits),
            selected_input_a.map(f32::to_bits)
        );
        state.output_channels = vec![2];
        assert_eq!(unsafe { state.process::<f32>(1) }, 4);
        assert_eq!(output_b, [0.0; 4]);
        assert_eq!(other_b, selected_input_b);
        state.buffers.remove(1);
        state.inputs = 1;
        state.input_channel = 1;
        state.output_channels = vec![1];
        state.dsp.get_mut().pitch = PitchShifter::new(0.0, true, 0.0).unwrap();
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
            ..Default::default()
        }
        .build()
        .unwrap();
        unsafe { state.process::<f32>(0) };
        assert!(output_a[0] > 0.0 && output_a[0] <= 0.892);
        assert_eq!(output_a[1], 0.0);
        assert_eq!(other_a, [0.0; 4]);
        state.buffers[1].buffers[0] = state.buffers[0].buffers[0];
        assert!(state.validate_pointers().is_err());

        // Exercise the wet route through the raw ASIO descriptors, including ring wraps.
        state.buffers[1].buffers[0] = output_a.as_mut_ptr().cast();
        state.dsp.get_mut().gate = GateSettings {
            enabled: false,
            threshold: crate::gate::GateThreshold::Auto,
            attack_ms: 5.0,
            release_ms: 80.0,
        }
        .build()
        .unwrap();
        state.dsp.get_mut().level = crate::level::LevelSettings::default().build().unwrap();
        state.dsp.get_mut().pitch = PitchShifter::new(3.0, true, 0.0).unwrap();
        for _ in 0..1024 {
            assert_eq!(unsafe { state.process::<f32>(0) }, 3);
        }
        assert_ne!(input_a.map(f32::to_bits), output_a.map(f32::to_bits));
        assert!(output_a.iter().all(|s| s.is_finite() && s.abs() <= 1.0));
        assert_eq!(other_a, [0.0; 4]);
        assert_eq!(output_b, input_b); // index 0 never writes index 1.
        for _ in 0..4096 {
            assert_eq!(unsafe { state.process::<f32>(1) }, 4);
        }
        assert!(output_b.iter().all(|s| (*s - 0.1).abs() < 0.001));
        assert_eq!(other_b, [0.0; 4]);
    }
}
