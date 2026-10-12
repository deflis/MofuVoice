# Repository Guidelines

## Product Goals & Development Priorities

- Build a voice changer for streamers, primarily targeting male-to-female voice conversion.
- Target a total monitoring latency of 10 ms or less from microphone input to headphone output, including input/output buffering, DSP algorithmic delay, and hardware conversion. Verify through actual conversation that monitoring the processed voice does not disrupt the user's thinking or speech.
- Consider implementing DSP features needed for streaming.
- Prioritize voice quality first. Improve processing speed after improving quality, while preserving the achieved quality.
- Do not consider using existing libraries.

## Testing Guidelines

Use adjacent unit tests and integration tests. Cover formats, mapping, transitions, phase continuity, formant correction, invalid arguments, and block boundaries. Preserve neutral bypass output bits; run DSP quality tests in Release. Measure processing time per pitch setting while preserving achieved quality.

Record hardware settings, overruns, sample-position discontinuities, and listening results locally. Distinguish DSP delay, reported driver latency, and measured round-trip latency.

## Documentation Guidelines

The `docs/` directory is for documentation intended for general users, such as setup instructions, usage guides, and troubleshooting. Do not place developer notes, implementation reports, work logs, or validation results there.

## Commit Guidelines

After each algorithm improvement, run the relevant validation and commit the changes.

Do not commit incidental reports, work logs, validation summaries, benchmark outputs, or listening notes. Keep these records locally. Commit documentation only when it is necessary for maintaining or using the project, or when explicitly requested by the user.
