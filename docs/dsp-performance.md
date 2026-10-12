# DSP performance validation — 2026-10-13

Baseline: 35aaf63. Windows x64, Rust 1.99.0, `rustc -C opt-level=3`.
CPU identifier: AMD64 Family 23 Model 113 Stepping 0, AuthenticAMD; 24 logical processors.
Offline processing, 48 kHz f32 mono, 512-frame blocks, auto level disabled.
Input: 113 Hz fundamental with third and seventh harmonics, as in `benches/spectral.rs`.
Both implementations are compiled into one comparison executable. Pin its thread to
logical CPU 4 (affinity mask 16), warm each processor with 4096 samples, process
48000 samples per measurement, alternate implementation order, and take the
median of 15 measurements per implementation and setting. No concurrent test or
build was running during the final timing comparison.

| Pitch | Preserve formants | Before ns/sample | After ns/sample | Time reduction |
| ---: | :---: | ---: | ---: | ---: |
| -12 | off | 2419.8 | 1850.5 | 23.53% |
| -12 | on | 4139.3 | 3361.7 | 18.79% |
| -3 | off | 2620.8 | 1977.9 | 24.53% |
| -3 | on | 5437.3 | 4401.4 | 19.05% |
| 0 | off | 3692.1 | 2875.0 | 22.13% |
| 0 | on | 4593.8 | 3784.6 | 17.61% |
| +3 | off | 6137.9 | 4790.0 | 21.96% |
| +3 | on | 5866.6 | 4853.1 | 17.28% |
| +7 | off | 4881.2 | 3111.3 | 36.26% |
| +7 | on | 6303.8 | 4837.1 | 23.27% |
| +12 | off | 3246.8 | 2417.0 | 25.56% |
| +12 | on | 5743.1 | 4154.1 | 27.67% |

Timing varies with host load and CPU frequency; the table is a measurement, not a
realtime deadline guarantee. Neutral processing still runs the DSP to maintain
transition state, while its public output remains a bit-exact dry bypass.

The changes share phase rotations across peak regions, use slice-based FFT
butterflies, and omit envelope/phase calculations for bins whose two destinations
are both discarded. FFT size, arithmetic order inside butterflies, windows,
formant estimator and correction limits are preserved.

A separate baseline-versus-final comparison processed 576000 output samples,
including pitch retargeting, formant shifts -12/0/+12, chirp/harmonic/broadband
input and 127-frame blocks. Maximum absolute f32 output error and RMS error were
both zero. Unit tests compare FFT output bits with the indexed reference and
phase locking with an independent polar reference. Release integration tests
cover formats, neutral bypass, mapping, phase continuity, formants, invalid
settings, boundaries, allocation freedom, and discarded above-Nyquist lobes.

Local raw records and comparison harnesses are in ignored `logs/`:
`pitch-before.rs`, `paired-pinned.rs`, `final-performance.txt`, `compare.rs`,
`final-compare.txt`, `completed-tests.txt`, and `completed-clippy.txt`.

DSP algorithmic delay remains 512 samples (10.667 ms at 48 kHz). No ASIO device
was started for this offline validation. Driver latency, hardware buffering,
overruns, sample-position discontinuities, measured round-trip latency and
conversation/listening results were not measured in this run.
