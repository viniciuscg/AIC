//! Catálogo de sons. Três origens:
//! - **sintetizados**: gerados pelo AIC na primeira vez (WAV em `~/.local/share/aic/sounds`), sem direito autoral;
//! - **baixados**: o `install.sh` baixa de sites de efeitos (não vão pro repo);
//! - **seus**: qualquer `.mp3/.ogg/.wav/.oga` que você colocar na pasta de sons.

use std::f32::consts::PI;
use std::io::Write;
use std::path::{Path, PathBuf};

use aic_core::paths;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Origin {
    Synth,
    Download,
    User,
}

#[derive(Debug, Clone)]
pub struct Sound {
    pub id: String,
    pub name: String,
    pub desc: String,
    pub origin: Origin,
}

struct Builtin {
    id: &'static str,
    name: &'static str,
    desc: &'static str,
    origin: Origin,
    /// Sintetizado: função que gera as amostras. Baixado: nome do arquivo na pasta de sons.
    synth: Option<fn() -> Vec<f32>>,
    file: &'static str,
}

const RATE: u32 = 44_100;

const BUILTINS: &[Builtin] = &[
    Builtin { id: "aic-acorde", name: "Acorde grunge", desc: "Power chord distorcido em Mi, afinado meio tom abaixo (sintetizado)", origin: Origin::Synth, synth: Some(grunge_chord), file: "aic-acorde.wav" },
    Builtin { id: "aic-feedback", name: "Microfonia", desc: "Nota longa com microfonia subindo, estilo fim de música (sintetizado)", origin: Origin::Synth, synth: Some(feedback), file: "aic-feedback.wav" },
    Builtin { id: "sino", name: "Sino", desc: "Sino curto e limpo (sintetizado)", origin: Origin::Synth, synth: Some(bell), file: "sino.wav" },
    Builtin { id: "pop", name: "Pop", desc: "Estalo discreto (sintetizado)", origin: Origin::Synth, synth: Some(pop), file: "pop.wav" },
    Builtin { id: "plankton-gemido", name: "Plankton gemendo", desc: "\"Aaaahhhhmmmm\" (baixado pelo install.sh)", origin: Origin::Download, synth: None, file: "plankton-gemido.mp3" },
];

/// Som padrão de cada tema.
pub fn default_for(theme: &str) -> &'static str {
    match theme {
        "plankton" => "plankton-gemido",
        _ => "aic-acorde",
    }
}

/// Todos os sons disponíveis agora (baixados só aparecem se o arquivo existir).
pub fn catalog() -> Vec<Sound> {
    let dir = paths::sounds_dir();
    let mut v: Vec<Sound> = BUILTINS
        .iter()
        .filter(|b| b.origin == Origin::Synth || dir.join(b.file).exists())
        .map(|b| Sound { id: b.id.into(), name: b.name.into(), desc: b.desc.into(), origin: b.origin })
        .collect();
    let known: Vec<&str> = BUILTINS.iter().map(|b| b.file).collect();
    let mut user: Vec<Sound> = std::fs::read_dir(&dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().to_string();
            let ext = Path::new(&name).extension()?.to_string_lossy().to_lowercase();
            (["mp3", "ogg", "oga", "wav", "flac"].contains(&ext.as_str()) && !known.contains(&name.as_str())).then(|| Sound {
                id: format!("arquivo:{name}"),
                name: Path::new(&name).file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or(name.clone()),
                desc: "seu arquivo".into(),
                origin: Origin::User,
            })
        })
        .collect();
    user.sort_by(|a, b| a.name.cmp(&b.name));
    v.extend(user);
    v
}

/// Id do catálogo (ou caminho de arquivo) → arquivo pronto pra tocar. Gera o sintetizado se faltar.
pub fn resolve(id: &str) -> Option<PathBuf> {
    let dir = paths::sounds_dir();
    if let Some(name) = id.strip_prefix("arquivo:") {
        let p = dir.join(name);
        return p.exists().then_some(p);
    }
    if let Some(b) = BUILTINS.iter().find(|b| b.id == id) {
        let p = dir.join(b.file);
        if p.exists() {
            return Some(p);
        }
        let synth = b.synth?;
        let _ = std::fs::create_dir_all(&dir);
        write_wav(&p, &synth()).ok()?;
        return Some(p);
    }
    let p = PathBuf::from(id);
    p.exists().then_some(p)
}

pub fn name_of(id: &str) -> String {
    catalog().into_iter().find(|s| s.id == id).map(|s| s.name).unwrap_or_else(|| id.to_string())
}

fn write_wav(path: &Path, samples: &[f32]) -> std::io::Result<()> {
    let data_len = (samples.len() * 2) as u32;
    let mut f = std::fs::File::create(path)?;
    f.write_all(b"RIFF")?;
    f.write_all(&(36 + data_len).to_le_bytes())?;
    f.write_all(b"WAVEfmt ")?;
    f.write_all(&16u32.to_le_bytes())?;
    f.write_all(&1u16.to_le_bytes())?; // PCM
    f.write_all(&1u16.to_le_bytes())?; // mono
    f.write_all(&RATE.to_le_bytes())?;
    f.write_all(&(RATE * 2).to_le_bytes())?;
    f.write_all(&2u16.to_le_bytes())?;
    f.write_all(&16u16.to_le_bytes())?;
    f.write_all(b"data")?;
    f.write_all(&data_len.to_le_bytes())?;
    let peak = samples.iter().fold(0.0f32, |m, s| m.max(s.abs())).max(1e-6);
    let mut buf = Vec::with_capacity(samples.len() * 2);
    for s in samples {
        buf.extend_from_slice(&(((s / peak) * 0.85 * i16::MAX as f32) as i16).to_le_bytes());
    }
    f.write_all(&buf)
}

fn saw(phase: f32) -> f32 {
    2.0 * (phase - phase.floor()) - 1.0
}

/// Power chord Eb (afinação meio tom abaixo, como o grunge de Seattle): fundamental, quinta e oitava,
/// dois "violões" levemente desafinados, distorção por saturação e um palm-mute no ataque.
fn grunge_chord() -> Vec<f32> {
    let dur = 1.6;
    let n = (RATE as f32 * dur) as usize;
    let root = 77.78; // Eb2
    let notes = [root, root * 1.4983, root * 2.0];
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let t = i as f32 / RATE as f32;
        let mut s = 0.0;
        for (k, f) in notes.iter().enumerate() {
            for detune in [-0.12, 0.12] {
                let ff = f + detune + (t * 5.5).sin() * 0.15;
                s += saw(ff * t + k as f32 * 0.13) * 0.5;
            }
        }
        let attack = (t / 0.004).min(1.0);
        let env = attack * (-t * 2.2).exp();
        let driven = (s * 3.5 * env).tanh();
        // passa-baixa de 1 polo pra tirar a aspereza
        out.push(driven);
    }
    lowpass(&mut out, 3200.0);
    out
}

/// Nota sustentada que vai virando microfonia (harmônico agudo crescendo).
fn feedback() -> Vec<f32> {
    let dur = 2.2;
    let n = (RATE as f32 * dur) as usize;
    let f0 = 155.56; // Eb3
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let t = i as f32 / RATE as f32;
        let body = saw(f0 * t) * (-t * 1.3).exp();
        let fb_amp = ((t - 0.5).max(0.0) / 1.4).min(1.0);
        let fb = (2.0 * PI * f0 * 4.0 * t + (t * 6.0).sin() * 0.3).sin() * fb_amp * 0.8;
        let fade = ((dur - t) / 0.25).min(1.0);
        out.push(((body + fb) * 2.2).tanh() * fade * (t / 0.005).min(1.0));
    }
    lowpass(&mut out, 4500.0);
    out
}

fn bell() -> Vec<f32> {
    let n = (RATE as f32 * 1.2) as usize;
    (0..n)
        .map(|i| {
            let t = i as f32 / RATE as f32;
            let partials = [(880.0, 1.0, 3.0), (880.0 * 2.76, 0.5, 5.0), (880.0 * 5.4, 0.25, 8.0)];
            partials.iter().map(|(f, a, d)| (2.0 * PI * f * t).sin() * a * (-t * d).exp()).sum::<f32>() * (t / 0.002).min(1.0)
        })
        .collect()
}

fn pop() -> Vec<f32> {
    let n = (RATE as f32 * 0.12) as usize;
    let mut phase = 0.0f32;
    (0..n)
        .map(|i| {
            let t = i as f32 / RATE as f32;
            let f = 650.0 - 450.0 * (t / 0.12);
            phase += f / RATE as f32;
            (2.0 * PI * phase).sin() * (-t * 35.0).exp()
        })
        .collect()
}

fn lowpass(x: &mut [f32], cutoff: f32) {
    let rc = 1.0 / (2.0 * PI * cutoff);
    let dt = 1.0 / RATE as f32;
    let a = dt / (rc + dt);
    let mut y = 0.0;
    for s in x.iter_mut() {
        y += a * (*s - y);
        *s = y;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sintetizados_geram_audio_valido() {
        for f in [grunge_chord, feedback, bell, pop] {
            let s = f();
            assert!(s.len() > 1000);
            assert!(s.iter().all(|x| x.is_finite()));
            assert!(s.iter().any(|x| x.abs() > 0.05));
        }
    }

    #[test]
    fn wav_tem_cabecalho_certo() {
        let p = std::env::temp_dir().join(format!("aic-som-{}.wav", std::process::id()));
        write_wav(&p, &pop()).unwrap();
        let b = std::fs::read(&p).unwrap();
        assert_eq!(&b[0..4], b"RIFF");
        assert_eq!(&b[8..12], b"WAVE");
        let _ = std::fs::remove_file(p);
    }
}
