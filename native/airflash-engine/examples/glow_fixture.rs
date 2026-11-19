//! Export the exact PCM fixture used by the Windows test to compare another sender.
use airflash_engine::glow_protocol;
use anyhow::{Context, Result};
use std::path::PathBuf;

fn main() -> Result<()> {
    let destination = PathBuf::from(std::env::args_os().nth(1).context("output directory required")?);
    std::fs::create_dir_all(&destination)?;
    let samples = glow_protocol::test_samples(44100);
    let mut wav = hound::WavWriter::create(destination.join("protocol-test.wav"), hound::WavSpec {
        channels:2, sample_rate:44100, bits_per_sample:16, sample_format:hound::SampleFormat::Int,
    })?;
    for sample in samples { wav.write_sample(sample)?; }
    wav.finalize()?;
    std::fs::write(destination.join("protocol.jpg"), glow_protocol::ART)?;
    Ok(())
}
