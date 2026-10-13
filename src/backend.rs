//! Control-thread boundary for native audio backends. No native handles escape this API.
use crate::audio::DspSettings;
use std::error::Error;

pub type Result<T> = std::result::Result<T, Box<dyn Error>>;

#[derive(Clone, Debug, Default, PartialEq)]
#[cfg_attr(feature = "desktop", derive(serde::Serialize))]
#[cfg_attr(feature = "desktop", serde(rename_all = "camelCase"))]
pub struct AudioTelemetry {
    pub blocks: u64,
    pub nonzero_samples: u64,
    pub max_callback_us: f64,
    pub deadline_misses: u64,
    pub timestamp_blocks: Option<u64>,
    pub sample_position_gaps: Option<u64>,
    pub fault_flags: u32,
    pub output_blocks: Option<u64>,
    pub underruns: Option<u64>,
    pub overruns: Option<u64>,
    pub queue_frames: Option<usize>,
    pub observed_input_frames: Option<usize>,
    pub observed_output_frames: Option<usize>,
    pub queue_peak_frames: Option<usize>,
    pub startup_queue_frames: Option<usize>,
    pub queue_capacity_frames: Option<usize>,
    pub max_input_callback_us: Option<f64>,
    pub max_output_callback_us: Option<f64>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AudioConfiguration {
    pub device: String,
    pub output_device: String,
    pub sample_rate: f64,
    pub inputs: usize,
    pub outputs: usize,
}

pub struct AudioInspection {
    pub configuration: AudioConfiguration,
    pub details: String,
}

/// Owned on the control thread; drop must stop callbacks before freeing their storage.
pub trait AudioStream {
    fn start(&mut self) -> Result<()>;
    fn stop(&mut self) -> Result<()>;
    fn blocks(&self) -> u64;
    fn has_playback(&self) -> bool {
        self.blocks() > 0
    }
    fn configuration(&self) -> AudioConfiguration;
    fn details(&self) -> String;
    fn telemetry(&self) -> AudioTelemetry;
    fn latency_ms(&self) -> Result<(Option<f64>, Option<f64>)>;
    fn summary(&self) -> String;
    fn latency_summary(&self) -> Result<String>;
    fn check_events(&self) -> Result<()>;
    fn check_test_result(&self) -> Result<()>;
    fn report(&self) {
        println!("{}", self.summary());
    }
    fn print_latencies(&self) -> Result<()> {
        println!("{}", self.latency_summary()?);
        Ok(())
    }
}

/// Implement native device discovery/configuration separately from shared DSP.
/// Inspect validates the requested 48 kHz period without starting audio callbacks.
pub trait AudioBackend {
    fn device_names(&self) -> Result<Vec<String>>;
    fn output_device_names(&self) -> Result<Vec<String>> {
        Ok(Vec::new())
    }
    fn inspect_configuration(&self, name: &str, frames: u32) -> Result<AudioInspection>;
    fn inspect(&self, name: &str, frames: u32) -> Result<String> {
        Ok(self.inspect_configuration(name, frames)?.details)
    }
    fn open(&self, name: &str, frames: u32, dsp: DspSettings) -> Result<Box<dyn AudioStream>>;
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum BackendKind {
    #[default]
    Auto,
    Cpal,
    Asio,
}
impl BackendKind {
    pub fn parse(value: &str) -> Result<Self> {
        match value {
            "auto" => Ok(Self::Auto),
            "cpal" => Ok(Self::Cpal),
            "asio" => Ok(Self::Asio),
            _ => Err("Backend must be auto, cpal or asio".into()),
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Cpal => "cpal",
            Self::Asio => "asio",
        }
    }
    pub fn available(self) -> bool {
        match self {
            Self::Auto => cfg!(feature = "cpal-backend") || cfg!(all(windows, feature = "asio")),
            Self::Cpal => cfg!(feature = "cpal-backend"),
            Self::Asio => cfg!(all(windows, feature = "asio")),
        }
    }
}

/// Prefer registered ASIO drivers when the direct adapter is compiled in. Opening
/// still validates the selected driver; do not silently switch a failing device.
fn prefer(kind: BackendKind, asio_registered: bool) -> BackendKind {
    match kind {
        BackendKind::Auto if asio_registered => BackendKind::Asio,
        BackendKind::Auto => BackendKind::Cpal,
        explicit => explicit,
    }
}
pub fn resolve(kind: BackendKind) -> BackendKind {
    if kind != BackendKind::Auto {
        return kind;
    }
    #[cfg(all(windows, feature = "asio"))]
    let registered = !asio_sys::Asio::new().driver_names().is_empty();
    #[cfg(not(all(windows, feature = "asio")))]
    let registered = false;
    prefer(kind, registered)
}

pub fn select(kind: BackendKind, output_device: &str) -> Result<Box<dyn AudioBackend>> {
    match resolve(kind) {
        BackendKind::Auto => unreachable!("auto resolves to a concrete backend"),
        BackendKind::Cpal => {
            #[cfg(feature = "cpal-backend")]
            {
                Ok(Box::new(crate::cpal_backend::CpalBackend::new(
                    output_device,
                )))
            }
            #[cfg(not(feature = "cpal-backend"))]
            {
                let _ = output_device;
                Err("CPAL backend is disabled; enable cpal-backend".into())
            }
        }
        BackendKind::Asio => {
            if !output_device.is_empty() {
                return Err(
                    "ASIO uses one duplex device; choose --backend cpal to use --output-device"
                        .into(),
                );
            }
            #[cfg(all(windows, feature = "asio"))]
            {
                Ok(Box::new(AsioBackend))
            }
            #[cfg(not(all(windows, feature = "asio")))]
            {
                Err("Direct ASIO requires Windows and the asio feature".into())
            }
        }
    }
}

pub fn default_backend() -> Result<Box<dyn AudioBackend>> {
    select(BackendKind::Auto, "")
}

#[cfg(all(windows, feature = "asio"))]
pub struct AsioBackend;

#[cfg(all(windows, feature = "asio"))]
fn load_driver(name: &str) -> Result<asio_sys::Driver> {
    let host = asio_sys::Asio::new();
    let name = host
        .driver_names()
        .into_iter()
        .find(|candidate| candidate.eq_ignore_ascii_case(name))
        .ok_or_else(|| format!("ASIO device '{name}' not found; use --list-devices"))?;
    Ok(host.load_driver(&name)?)
}

#[cfg(all(windows, feature = "asio"))]
impl AudioBackend for AsioBackend {
    fn device_names(&self) -> Result<Vec<String>> {
        Ok(asio_sys::Asio::new().driver_names())
    }
    fn inspect_configuration(&self, name: &str, frames: u32) -> Result<AudioInspection> {
        let driver = load_driver(name)?;
        let config = crate::asio::Configuration::query(&driver)?;
        config.validate(frames)?;
        let mut configuration = config.audio_configuration();
        configuration.device = name.to_string();
        configuration.output_device = name.to_string();
        Ok(AudioInspection {
            configuration,
            details: config.summary(),
        })
    }
    fn open(&self, name: &str, frames: u32, dsp: DspSettings) -> Result<Box<dyn AudioStream>> {
        let driver = load_driver(name)?;
        let config = crate::asio::Configuration::query(&driver)?;
        let mut engine = crate::asio::Engine::open(driver, config, frames, dsp)?;
        engine.set_device_name(name);
        Ok(Box::new(engine))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn auto_prefers_asio_and_explicit_selection_is_respected() {
        assert_eq!(prefer(BackendKind::Auto, true), BackendKind::Asio);
        assert_eq!(prefer(BackendKind::Auto, false), BackendKind::Cpal);
        assert_eq!(prefer(BackendKind::Cpal, true), BackendKind::Cpal);
        assert_eq!(prefer(BackendKind::Asio, false), BackendKind::Asio);
    }
}
