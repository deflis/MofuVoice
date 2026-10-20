// Keep each supported window pair visible to libtest so expensive quality
// matrices can run across the runner's available test threads.
macro_rules! window_cases {
    ($($test:ident),+ $(,)?) => {
        window_cases!(@cases [$($test),+];
            (analysis_1024_synthesis_256, 1024, 256),
            (analysis_1024_synthesis_512, 1024, 512),
            (analysis_1024_synthesis_1024, 1024, 1024),
            (analysis_2048_synthesis_256, 2048, 256),
            (analysis_2048_synthesis_512, 2048, 512),
            (analysis_2048_synthesis_1024, 2048, 1024),
            (analysis_2048_synthesis_2048, 2048, 2048),
            (analysis_4096_synthesis_256, 4096, 256),
            (analysis_4096_synthesis_512, 4096, 512),
            (analysis_4096_synthesis_1024, 4096, 1024),
            (analysis_4096_synthesis_2048, 4096, 2048),
            (analysis_8192_synthesis_256, 8192, 256),
            (analysis_8192_synthesis_512, 8192, 512),
            (analysis_8192_synthesis_1024, 8192, 1024),
            (analysis_8192_synthesis_2048, 8192, 2048),
        );
    };
    (@cases $tests:tt; $(($name:ident, $analysis:expr, $synthesis:expr)),+ $(,)?) => {
        $(mod $name {
            window_cases!(@tests $tests; $analysis, $synthesis);
        })+
    };
    (@tests [$($test:ident),+]; $analysis:expr, $synthesis:expr) => {
        $(#[test]
        fn $test() {
            super::$test(rust_low_latency_voice_changer::pitch::SpectralWindows {
                analysis: $analysis,
                synthesis: $synthesis,
            });
        })+
    };
}
