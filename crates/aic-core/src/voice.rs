//! Voz do Layne: lista os microfones, grava um `.wav` e transcreve.
//!
//! Gravação: `parecord` (PulseAudio/PipeWire), senão `pw-record`, senão `arecord` (este ignora a
//! escolha de microfone). 16 kHz mono, o que o Whisper espera. Parar = SIGINT, que fecha o
//! cabeçalho do WAV direito.
//!
//! Transcrição (`[layne.voice]` no config): whisper.cpp local, API no formato da OpenAI
//! (OpenAI, Groq…) ou um comando seu.

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};

use crate::config::{Transcriber, VoiceConfig};
use crate::paths;

#[derive(Debug, Clone, PartialEq)]
pub struct Mic {
    /// Nome da fonte (`alsa_input.usb-…`), o que vai no config.
    pub name: String,
    /// Nome legível ("EMEET PIXY 2K Mono").
    pub desc: String,
}

fn has(bin: &str) -> bool {
    Command::new("sh")
        .arg("-c")
        .arg(format!("command -v {bin}"))
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

/// Microfones (fontes de entrada; os "monitor" das saídas ficam de fora).
pub fn list_mics() -> Vec<Mic> {
    let Ok(out) = Command::new("pactl")
        .args(["-f", "json", "list", "sources"])
        .stderr(Stdio::null())
        .output()
    else {
        return vec![];
    };
    let Ok(v) = serde_json::from_slice::<serde_json::Value>(&out.stdout) else {
        return vec![];
    };
    v.as_array()
        .into_iter()
        .flatten()
        .filter_map(|s| {
            let name = s.get("name")?.as_str()?.to_string();
            if name.ends_with(".monitor")
                || s.pointer("/monitor_source")
                    .and_then(|m| m.as_str())
                    .is_some_and(|m| !m.is_empty() && m != "n/a")
            {
                return None;
            }
            let desc = s
                .get("description")
                .and_then(|d| d.as_str())
                .filter(|d| *d != "(null)")
                .unwrap_or(&name)
                .to_string();
            Some(Mic { name, desc })
        })
        .collect()
}

/// Microfone padrão do sistema.
pub fn default_mic() -> Option<String> {
    let out = Command::new("pactl")
        .arg("get-default-source")
        .stderr(Stdio::null())
        .output()
        .ok()?;
    let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
    (!s.is_empty()).then_some(s)
}

/// Programa + args que gravam `file` do microfone `mic`.
pub fn record_command(
    mic: Option<&str>,
    file: &Path,
    available: impl Fn(&str) -> bool,
) -> Option<(String, Vec<String>)> {
    let f = file.to_string_lossy().to_string();
    if available("parecord") {
        let mut a = vec![
            "--rate=16000".into(),
            "--channels=1".into(),
            "--format=s16le".into(),
            "--file-format=wav".into(),
        ];
        if let Some(m) = mic {
            a.push(format!("--device={m}"));
        }
        a.push(f);
        return Some(("parecord".into(), a));
    }
    if available("pw-record") {
        let mut a = vec![
            "--rate".into(),
            "16000".into(),
            "--channels".into(),
            "1".into(),
        ];
        if let Some(m) = mic {
            a.extend(["--target".into(), m.to_string()]);
        }
        a.push(f);
        return Some(("pw-record".into(), a));
    }
    if available("arecord") {
        return Some((
            "arecord".into(),
            vec![
                "-q".into(),
                "-f".into(),
                "S16_LE".into(),
                "-r".into(),
                "16000".into(),
                "-c".into(),
                "1".into(),
                f,
            ],
        ));
    }
    None
}

/// Uma gravação em andamento.
pub struct Recording {
    child: Child,
    pub file: PathBuf,
    pub started: Instant,
    pub mic: Option<String>,
}

impl Recording {
    pub fn start(mic: Option<&str>) -> Result<Self> {
        let dir = paths::state_dir().join("voice");
        std::fs::create_dir_all(&dir)?;
        let file = dir.join(format!(
            "{}.wav",
            chrono::Local::now().format("%Y%m%d-%H%M%S")
        ));
        let Some((prog, args)) = record_command(mic, &file, has) else {
            bail!(
                "nenhum gravador encontrado (instale pulseaudio-utils, pipewire-bin ou alsa-utils)"
            );
        };
        let child = Command::new(&prog)
            .args(&args)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .with_context(|| format!("abrindo {prog}"))?;
        Ok(Self {
            child,
            file,
            started: Instant::now(),
            mic: mic.map(String::from),
        })
    }

    pub fn elapsed(&self) -> Duration {
        self.started.elapsed()
    }

    /// O gravador morreu sozinho (microfone sumiu, permissão…)? Devolve o erro.
    pub fn failed(&mut self) -> Option<String> {
        match self.child.try_wait() {
            Ok(Some(status)) if !status.success() => {
                let mut err = String::new();
                if let Some(mut e) = self.child.stderr.take() {
                    use std::io::Read;
                    let _ = e.read_to_string(&mut err);
                }
                Some(if err.trim().is_empty() {
                    format!("o gravador saiu ({status})")
                } else {
                    err.trim().to_string()
                })
            }
            _ => None,
        }
    }

    /// Para (SIGINT, pra fechar o WAV) e devolve o arquivo.
    pub fn stop(mut self) -> Result<PathBuf> {
        let _ = Command::new("kill")
            .args(["-INT", &self.child.id().to_string()])
            .status();
        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            if self.child.try_wait()?.is_some() {
                break;
            }
            if Instant::now() > deadline {
                let _ = self.child.kill();
                let _ = self.child.wait();
                break;
            }
            std::thread::sleep(Duration::from_millis(30));
        }
        let len = std::fs::metadata(&self.file).map(|m| m.len()).unwrap_or(0);
        // 44 bytes de cabeçalho + ~0.2s de áudio
        if len < 44 + 6_400 {
            let _ = std::fs::remove_file(&self.file);
            bail!("gravação vazia ou curta demais");
        }
        Ok(self.file)
    }

    /// Descarta a gravação.
    pub fn cancel(mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = std::fs::remove_file(&self.file);
    }
}

/// `{file}` e `{lang}` do comando próprio, com aspas simples de shell.
pub fn fill_command(template: &str, file: &Path, lang: &str) -> String {
    let q = |s: &str| format!("'{}'", s.replace('\'', r"'\''"));
    template
        .replace("{file}", &q(&file.to_string_lossy()))
        .replace("{lang}", &q(lang))
}

/// Limpa o que os transcritores costumam devolver a mais (marcações, espaços, linhas).
pub fn clean(text: &str) -> String {
    let t: Vec<&str> = text
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !matches!(*l, "[BLANK_AUDIO]" | "[MUSIC]" | "(música)"))
        .collect();
    t.join(" ").split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Transcreve o `.wav` (bloqueia: rode numa thread). Apaga o arquivo no fim, deu certo ou não.
pub fn transcribe(cfg: &VoiceConfig, file: &Path) -> Result<String> {
    let r = transcribe_keep(cfg, file);
    let _ = std::fs::remove_file(file);
    let text = clean(&r?);
    if text.is_empty() {
        bail!("não entendi nada no áudio");
    }
    Ok(text)
}

fn transcribe_keep(cfg: &VoiceConfig, file: &Path) -> Result<String> {
    let lang = cfg.language.trim();
    match cfg.transcriber {
        Transcriber::WhisperCpp => {
            let model = cfg.model_path();
            if !model.exists() {
                bail!(
                    "modelo do whisper.cpp não encontrado em {} (Config → Layne)",
                    paths::pretty(&model)
                );
            }
            let mut c = Command::new(&cfg.whisper_bin);
            c.arg("-m")
                .arg(&model)
                .arg("-f")
                .arg(file)
                .args(["-nt", "-np"]);
            c.args(["-l", if lang.is_empty() { "auto" } else { lang }]);
            let out = c
                .output()
                .with_context(|| format!("rodando {} (instalado?)", cfg.whisper_bin))?;
            if !out.status.success() {
                bail!(
                    "{} falhou: {}",
                    cfg.whisper_bin,
                    String::from_utf8_lossy(&out.stderr)
                        .lines()
                        .last()
                        .unwrap_or("")
                );
            }
            Ok(String::from_utf8_lossy(&out.stdout).to_string())
        }
        Transcriber::Api => {
            let key = std::env::var(&cfg.api_key_env)
                .ok()
                .filter(|k| !k.trim().is_empty())
                .with_context(|| format!("defina {} no ~/.config/aic/.env", cfg.api_key_env))?;
            let part = reqwest::blocking::multipart::Part::bytes(std::fs::read(file)?)
                .file_name("audio.wav")
                .mime_str("audio/wav")?;
            let mut form = reqwest::blocking::multipart::Form::new()
                .part("file", part)
                .text("model", cfg.api_model.clone())
                .text("response_format", "text");
            if !lang.is_empty() {
                form = form.text("language", lang.to_string());
            }
            let resp = reqwest::blocking::Client::builder()
                .timeout(Duration::from_secs(120))
                .build()?
                .post(&cfg.api_url)
                .bearer_auth(key.trim())
                .multipart(form)
                .send()
                .context("chamando a API de transcrição")?;
            let status = resp.status();
            let body = resp.text().unwrap_or_default();
            if !status.is_success() {
                bail!(
                    "API respondeu {status}: {}",
                    body.chars().take(200).collect::<String>()
                );
            }
            // response_format=text devolve texto puro; algumas APIs ignoram e mandam JSON
            Ok(serde_json::from_str::<serde_json::Value>(&body)
                .ok()
                .and_then(|v| v.get("text")?.as_str().map(String::from))
                .unwrap_or(body))
        }
        Transcriber::Command => {
            if cfg.command.trim().is_empty() {
                bail!("defina o comando de transcrição (Config → Layne)");
            }
            let cmd = fill_command(&cfg.command, file, lang);
            let out = Command::new("sh")
                .arg("-c")
                .arg(&cmd)
                .output()
                .context("rodando o comando de transcrição")?;
            if !out.status.success() {
                bail!(
                    "o comando saiu com {}: {}",
                    out.status,
                    String::from_utf8_lossy(&out.stderr)
                        .lines()
                        .last()
                        .unwrap_or("")
                );
            }
            Ok(String::from_utf8_lossy(&out.stdout).to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gravador_preferido_e_microfone() {
        let f = Path::new("/tmp/a.wav");
        let (p, a) = record_command(Some("alsa_input.usb-x"), f, |_| true).unwrap();
        assert_eq!(p, "parecord");
        assert!(a.contains(&"--device=alsa_input.usb-x".to_string()));
        assert_eq!(a.last().unwrap(), "/tmp/a.wav");
        let (p, a) = record_command(Some("m"), f, |b| b == "pw-record").unwrap();
        assert_eq!(p, "pw-record");
        assert!(a.windows(2).any(|w| w == ["--target", "m"]));
        let (p, _) = record_command(None, f, |b| b == "arecord").unwrap();
        assert_eq!(p, "arecord");
        assert!(record_command(None, f, |_| false).is_none());
    }

    #[test]
    fn comando_proprio_com_aspas() {
        let c = fill_command(
            "whisper {file} --language {lang}",
            Path::new("/tmp/it's.wav"),
            "pt",
        );
        assert_eq!(c, r"whisper '/tmp/it'\''s.wav' --language 'pt'");
    }

    #[test]
    fn limpa_saida() {
        assert_eq!(
            clean("  [BLANK_AUDIO]\n Cria um workspace\n  pessoal  \n"),
            "Cria um workspace pessoal"
        );
        assert_eq!(clean("[BLANK_AUDIO]"), "");
    }
}
