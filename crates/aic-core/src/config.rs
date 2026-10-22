//! `~/.config/aic/config.toml`. Tudo tem default; o arquivo só guarda o que mudou.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::modules::{self, Flags, Module};
use crate::paths;

/// Como as sessões aparecem. Escolhida no primeiro uso; muda em Config ou pelo atalho.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum SessionView {
    /// Lista de blocos: cada bloco (contexto) com seus terminais.
    #[default]
    Blocks,
    /// Um bloco por aba, com os terminais divididos (o mais parecido com tmux).
    Tabs,
    /// Um terminal grande + lista lateral.
    Focus,
    /// Todos os terminais do workspace lado a lado.
    Grid,
}

impl SessionView {
    pub const ALL: [SessionView; 4] = [Self::Blocks, Self::Tabs, Self::Focus, Self::Grid];

    pub fn label(self) -> &'static str {
        match self {
            Self::Blocks => "Blocos",
            Self::Tabs => "Abas",
            Self::Focus => "Foco",
            Self::Grid => "Grade",
        }
    }

    pub fn desc(self) -> &'static str {
        match self {
            Self::Blocks => "Cada contexto é um bloco com seus terminais, um embaixo do outro.",
            Self::Tabs => "Um bloco por aba, terminais divididos lado a lado. O jeito tmux.",
            Self::Focus => "Um terminal grande e a lista de sessões ao lado, com prévia.",
            Self::Grid => "Todos os terminais do workspace numa grade.",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Config {
    /// Primeiro uso concluído (assistente de visualização/módulos).
    pub setup_done: bool,
    /// Visualização das sessões.
    pub view: SessionView,
    /// Módulos ligados/desligados.
    pub modules: Flags,
    /// Tecla líder: aperte e veja o catálogo de atalhos (which-key).
    pub leader: String,
    /// Prefixo estilo tmux: aperte, solte e aperte a tecla da ação (`Ctrl+B x` fecha o que está em foco).
    pub prefix: String,
    pub font_family: String,
    pub font_size: f32,
    pub ui_scale: f32,
    /// Pastas onde procurar repos (subpastas com `.git`), pra `#repo` e pra abrir sessões.
    pub repos_dirs: Vec<PathBuf>,
    pub sound: SoundConfig,
    pub notify: NotifyConfig,
    /// Mascote animado e partículas.
    pub animations: bool,
    pub layne: LayneConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct SoundConfig {
    pub enabled: bool,
    /// Id do som de "terminou" no catálogo (ex: `aic-acorde`) ou caminho de arquivo.
    /// `None` = o som padrão.
    pub done: Option<String>,
    /// Id/caminho do som de "precisa de você". `None` = sem som.
    pub attention: Option<String>,
    pub volume: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct NotifyConfig {
    /// Notificação do sistema (notify-send).
    pub desktop: bool,
    /// Só notifica/toca se a janela estiver sem foco ou a sessão não estiver visível.
    pub only_when_unfocused: bool,
}

/// Layne: o assistente pessoal (módulo `layne`). Um Claude com o agente `aic:layne`, sempre aberto.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct LayneConfig {
    /// Sobe a sessão junto com o app (senão, na primeira vez que o painel abre).
    pub auto_start: bool,
    /// Modelo do Claude (`--model`). Vazio = o padrão da conta.
    pub model: Option<String>,
    /// Largura do painel lateral (px).
    pub width: f32,
    pub voice: VoiceConfig,
}

impl Default for LayneConfig {
    fn default() -> Self {
        Self { auto_start: true, model: None, width: 480.0, voice: VoiceConfig::default() }
    }
}

/// Quem transforma o áudio em texto.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Transcriber {
    /// whisper.cpp local (`whisper-cli`): nada sai da máquina.
    #[default]
    WhisperCpp,
    /// API compatível com a da OpenAI (`/audio/transcriptions`): OpenAI, Groq…
    Api,
    /// Um comando seu: `{file}` vira o caminho do .wav, `{lang}` o idioma; o stdout é o texto.
    Command,
}

impl Transcriber {
    pub const ALL: [Transcriber; 3] = [Self::WhisperCpp, Self::Api, Self::Command];

    pub fn label(self) -> &'static str {
        match self {
            Self::WhisperCpp => "whisper.cpp (local)",
            Self::Api => "API (OpenAI, Groq…)",
            Self::Command => "comando próprio",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct VoiceConfig {
    /// Fonte do PulseAudio/PipeWire (`pactl list short sources`). Vazio = o microfone padrão do sistema.
    pub mic: Option<String>,
    pub transcriber: Transcriber,
    /// Idioma falado (código ISO: `pt`, `en`). Vazio = detectar.
    pub language: String,
    /// Binário do whisper.cpp.
    pub whisper_bin: String,
    /// Modelo `ggml-*.bin` do whisper.cpp. Vazio = `~/.local/share/aic/whisper/ggml-small.bin`.
    pub whisper_model: Option<PathBuf>,
    /// Endpoint de transcrição (formato da OpenAI).
    pub api_url: String,
    pub api_model: String,
    /// Variável de ambiente com a chave (fica no `~/.config/aic/.env`, nunca no config.toml).
    pub api_key_env: String,
    /// Comando próprio (transcriber = command).
    pub command: String,
    /// Manda a mensagem sozinho depois de transcrever (senão só cola e espera o Enter).
    pub auto_send: bool,
    /// Limite de uma gravação (s).
    pub max_seconds: u32,
}

impl Default for VoiceConfig {
    fn default() -> Self {
        Self {
            mic: None,
            transcriber: Transcriber::WhisperCpp,
            language: "pt".into(),
            whisper_bin: "whisper-cli".into(),
            whisper_model: None,
            api_url: "https://api.openai.com/v1/audio/transcriptions".into(),
            api_model: "gpt-4o-mini-transcribe".into(),
            api_key_env: "OPENAI_API_KEY".into(),
            command: String::new(),
            auto_send: true,
            max_seconds: 300,
        }
    }
}

impl VoiceConfig {
    pub fn model_path(&self) -> PathBuf {
        self.whisper_model.clone().unwrap_or_else(|| paths::data_dir().join("whisper/ggml-small.bin"))
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            setup_done: false,
            view: SessionView::Blocks,
            modules: Flags::new(),
            leader: "Ctrl+Space".into(),
            prefix: "Ctrl+B".into(),
            font_family: "JetBrainsMono Nerd Font Mono".into(),
            font_size: 14.0,
            ui_scale: 1.0,
            repos_dirs: vec![paths::home_dir()],
            sound: SoundConfig::default(),
            notify: NotifyConfig::default(),
            animations: true,
            layne: LayneConfig::default(),
        }
    }
}

impl Default for SoundConfig {
    fn default() -> Self {
        Self { enabled: true, done: None, attention: None, volume: 1.0 }
    }
}

impl Default for NotifyConfig {
    fn default() -> Self {
        Self { desktop: true, only_when_unfocused: true }
    }
}

impl Config {
    pub fn path() -> PathBuf {
        paths::config_dir().join("config.toml")
    }

    pub fn load() -> Self {
        std::fs::read_to_string(Self::path())
            .ok()
            .and_then(|t| match toml::from_str::<Self>(&t) {
                Ok(c) => Some(c),
                Err(e) => {
                    eprintln!("aic: config.toml inválido ({e}); usando padrões");
                    None
                }
            })
            .unwrap_or_default()
            .with_env()
    }

    /// Overrides do ambiente (e do `~/.config/aic/.env`, já carregado por `paths::load_dotenv`).
    /// Vazio = ignora. Veja `.env.example`.
    pub fn with_env(mut self) -> Self {
        let env = |k: &str| std::env::var(k).ok().map(|v| v.trim().to_string()).filter(|v| !v.is_empty());
        if let Some(v) = env("AIC_REPOS_DIRS") {
            let home = paths::home_dir();
            self.repos_dirs = v
                .split(':')
                .map(str::trim)
                .filter(|d| !d.is_empty())
                .map(|d| match d.strip_prefix("~/") {
                    Some(rest) => home.join(rest),
                    None if d == "~" => home.clone(),
                    None => PathBuf::from(d),
                })
                .collect();
        }
        self
    }

    pub fn save(&self) -> anyhow::Result<()> {
        let path = Self::path();
        if let Some(d) = path.parent() {
            std::fs::create_dir_all(d)?;
        }
        let tmp = path.with_extension("toml.tmp");
        std::fs::write(&tmp, toml::to_string_pretty(self)?)?;
        std::fs::rename(tmp, path)?;
        Ok(())
    }

    /// Módulo ligado?
    pub fn on(&self, m: Module) -> bool {
        modules::enabled(m, &self.modules)
    }

    /// Repos (nome, caminho) nas pastas de `repos_dirs`. Worktrees (`x.worktrees`) ficam de fora.
    pub fn repo_paths(&self) -> Vec<(String, PathBuf)> {
        let mut v: Vec<(String, PathBuf)> = Vec::new();
        for dir in &self.repos_dirs {
            for e in std::fs::read_dir(dir).into_iter().flatten().flatten() {
                let name = e.file_name().to_string_lossy().to_string();
                if name.starts_with('.') || name.ends_with(".worktrees") || !e.path().join(".git").exists() {
                    continue;
                }
                if !v.iter().any(|(n, _)| *n == name) {
                    v.push((name, e.path()));
                }
            }
        }
        v.sort();
        v
    }

    /// Nomes dos repos, pra autocompletar `#repo`.
    pub fn repos(&self) -> Vec<String> {
        self.repo_paths().into_iter().map(|(n, _)| n).collect()
    }

    pub fn repo_path(&self, name: &str) -> Option<PathBuf> {
        self.repo_paths().into_iter().find(|(n, _)| n == name).map(|(_, p)| p)
    }

    /// Nome do repo de um caminho (dentro de uma das `repos_dirs`; worktree → repo de origem).
    pub fn repo_of(&self, cwd: &Path) -> Option<String> {
        self.repos_dirs.iter().find_map(|d| {
            let rel = cwd.strip_prefix(d).ok()?;
            let first = rel.components().next()?.as_os_str().to_string_lossy().to_string();
            let name = first.trim_end_matches(".worktrees").to_string();
            (!name.is_empty()).then_some(name)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Config de antes (com temas, Notion, Slack) continua abrindo: os campos velhos são ignorados.
    #[test]
    fn config_antiga_carrega() {
        let old = "theme = \"plankton\"\nsetup_done = true\nleader = \"Ctrl+Space\"\nconnector = \"token\"\n\n[modules]\ntasks = false\nboard = false\nnotion = false\n\n[notion]\ntasks_database = \"x\"\n\n[slack]\nworkspace_url = \"y\"\n\n[themes.aic]\nvariant = \"jar\"\n";
        let c: Config = toml::from_str(old).expect("config antiga");
        assert!(c.setup_done);
        assert_eq!(c.prefix, "Ctrl+B", "prefixo padrão entra");
        assert!(!c.on(Module::Tasks));
        assert!(c.on(Module::Worktrees));
    }
}
