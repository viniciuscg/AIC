//! O som do app: um sabre de luz sintetizado aqui mesmo (não é gravação do filme, então não
//! tem direito autoral nenhum). Um arquivo seu entra no lugar em `[sound] file`.

use std::f32::consts::TAU;
use std::io::Write;
use std::path::{Path, PathBuf};

use aic_core::config::SoundConfig;
use aic_core::paths;

const RATE: u32 = 44_100;
/// Sobe o nome quando a síntese mudar: o arquivo antigo fica pra trás e o novo é gerado.
const FILE: &str = "sabre.wav";

/// O arquivo pra tocar: o seu, se existir; senão o sabre (gerado na primeira vez).
pub fn path(cfg: &SoundConfig) -> Option<PathBuf> {
    if let Some(f) = cfg.file.as_ref().filter(|f| f.exists()) {
        return Some(f.clone());
    }
    let dir = paths::sounds_dir();
    let p = dir.join(FILE);
    if !p.exists() {
        std::fs::create_dir_all(&dir).ok()?;
        write_wav(&p, &saber()).ok()?;
    }
    Some(p)
}

/// Ignição de sabre: estalo e chiado subindo nos primeiros 200 ms, depois o zumbido grave
/// (duas notas quase iguais batendo, com um leve vibrato), que some no fim.
fn saber() -> Vec<f32> {
    let dur = 1.15;
    let n = (RATE as f32 * dur) as usize;
    let dt = 1.0 / RATE as f32;
    let mut out = Vec::with_capacity(n);
    let (mut p1, mut p2, mut p3, mut sweep) = (0.0f32, 0.0f32, 0.0f32, 0.0f32);
    let (mut seed, mut lp, mut hp_prev) = (0x2545_f491u32, 0.0f32, 0.0f32);
    for i in 0..n {
        let t = i as f32 * dt;
        // o tom sobe rápido até o zumbido e fica ali
        let rise = 1.0 - (-t / 0.06).exp();
        let vibrato = 1.0 + 0.006 * (TAU * 6.0 * t).sin();
        let f = (48.0 + 62.0 * rise) * vibrato;
        p1 += f * dt;
        p2 += f * 1.012 * dt;
        p3 += f * 1.5 * dt;
        let saw = |p: f32| 2.0 * (p - p.floor()) - 1.0;
        let hum = saw(p1) * 0.5 + saw(p2) * 0.5 + (TAU * p3).sin() * 0.25;

        // chiado da ignição: ruído que abre e fecha
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        let noise = seed as f32 / u32::MAX as f32 * 2.0 - 1.0;
        let hiss_env = (t / 0.012).min(1.0) * (-t / 0.09).exp();
        let a = 0.35 * (1.0 - (-t / 0.05).exp()) + 0.05;
        lp += a * (noise - lp);
        let hiss = (lp - hp_prev) * hiss_env * 1.8;
        hp_prev = lp;

        // o "vuuum" subindo
        sweep += (180.0 + 1500.0 * (t / 0.2).min(1.0)) * dt;
        let whoosh = (TAU * sweep).sin() * (-t / 0.07).exp() * 0.35;

        let body = (t / 0.02).min(1.0) * ((dur - t) / 0.35).clamp(0.0, 1.0).powi(2);
        out.push(((hum * body * 1.4).tanh() * 0.7 + hiss + whoosh).clamp(-1.0, 1.0));
    }
    lowpass(&mut out, 5200.0);
    out
}

fn lowpass(x: &mut [f32], cutoff: f32) {
    let rc = 1.0 / (TAU * cutoff);
    let dt = 1.0 / RATE as f32;
    let a = dt / (rc + dt);
    let mut y = 0.0;
    for s in x.iter_mut() {
        y += a * (*s - y);
        *s = y;
    }
}

fn write_wav(path: &Path, samples: &[f32]) -> std::io::Result<()> {
    let data_len = (samples.len() * 2) as u32;
    let mut buf = Vec::with_capacity(44 + samples.len() * 2);
    buf.extend_from_slice(b"RIFF");
    buf.extend_from_slice(&(36 + data_len).to_le_bytes());
    buf.extend_from_slice(b"WAVEfmt ");
    buf.extend_from_slice(&16u32.to_le_bytes());
    buf.extend_from_slice(&1u16.to_le_bytes()); // PCM
    buf.extend_from_slice(&1u16.to_le_bytes()); // mono
    buf.extend_from_slice(&RATE.to_le_bytes());
    buf.extend_from_slice(&(RATE * 2).to_le_bytes());
    buf.extend_from_slice(&2u16.to_le_bytes());
    buf.extend_from_slice(&16u16.to_le_bytes());
    buf.extend_from_slice(b"data");
    buf.extend_from_slice(&data_len.to_le_bytes());
    let peak = samples.iter().fold(0.0f32, |m, s| m.max(s.abs())).max(1e-6);
    for s in samples {
        buf.extend_from_slice(&(((s / peak) * 0.85 * i16::MAX as f32) as i16).to_le_bytes());
    }
    std::fs::File::create(path)?.write_all(&buf)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Energia numa frequência (Goertzel) num trecho.
    fn power(x: &[f32], f: f32) -> f32 {
        let w = TAU * f / RATE as f32;
        let (mut s1, mut s2) = (0.0f32, 0.0f32);
        for v in x {
            let s = v + 2.0 * w.cos() * s1 - s2;
            s2 = s1;
            s1 = s;
        }
        (s1 * s1 + s2 * s2 - 2.0 * w.cos() * s1 * s2) / x.len() as f32
    }

    #[test]
    fn sabre_tem_zumbido_grave_e_chiado_no_comeco() {
        let s = saber();
        assert!(s.iter().all(|x| x.is_finite()));
        let at = |sec: f32| (sec * RATE as f32) as usize;
        let hum = &s[at(0.4)..at(0.7)];
        assert!(
            power(hum, 110.0) > 20.0 * power(hum, 3000.0),
            "zumbido grave"
        );
        let start = &s[..at(0.1)];
        let rms = |x: &[f32]| (x.iter().map(|v| v * v).sum::<f32>() / x.len() as f32).sqrt();
        assert!(rms(start) > 0.05, "ignição audível");
        // termina em silêncio, sem estalo
        assert!(s[s.len() - 200..].iter().all(|v| v.abs() < 0.02));
    }

    #[test]
    fn wav_tem_cabecalho_certo() {
        let p = std::env::temp_dir().join(format!("aic-som-{}.wav", std::process::id()));
        write_wav(&p, &[0.0, 0.5, -0.5]).unwrap();
        let b = std::fs::read(&p).unwrap();
        assert_eq!(&b[0..4], b"RIFF");
        assert_eq!(&b[8..12], b"WAVE");
        let _ = std::fs::remove_file(p);
    }
}
