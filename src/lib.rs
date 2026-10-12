//! Shared audio engine and application support. DSP modules are audio-API-independent.
pub mod asio;
pub mod cli;
pub mod gate;
#[cfg(feature = "desktop")]
pub mod gui;
#[cfg(feature = "desktop")]
mod gui_settings;
pub mod level;
pub mod native_spectrum;
pub mod pitch;
pub mod session;
pub mod spectrum;
pub const RATE: u32 = 48_000;
pub const DEFAULT_FRAMES: u32 = 512;
pub const TARGET_FRAMES: u32 = 64;

/// Input and output must have equal lengths. Returns nonzero input sample count.
pub fn bypass_block<T: Copy + Default + PartialEq>(input: &[T], output: &mut [T]) -> u64 {
    output.copy_from_slice(input);
    input.iter().filter(|&&value| value != T::default()).count() as u64
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn float_bits_and_zero_sample_delay() {
        let input = [1.0_f32, -0.0, 0.25, -0.5, f32::from_bits(1)];
        let mut output = [0.0; 5];
        assert_eq!(bypass_block(&input, &mut output), 4);
        assert_eq!(input.map(f32::to_bits), output.map(f32::to_bits));
    }
    #[test]
    fn integer_bits_and_block_boundary() {
        let input = [i32::MIN, -1, 0, 1, i32::MAX];
        let mut output = [0; 5];
        assert_eq!(bypass_block(&input, &mut output), 4);
        assert_eq!(input, output);
        assert_eq!(bypass_block(&[7], &mut output[..1]), 1);
        assert_eq!(output[0], 7);
    }
}
