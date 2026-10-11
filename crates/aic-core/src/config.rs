//! `~/.config/aic/config.toml`. Tudo tem default; o arquivo só guarda o que mudou.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::modules::{self, Flags, Module};
use crate::paths;

/// Como as sessões aparecem. Escolhida no primeiro uso; muda em Config ou pelo atalho.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum SessionView {
    /// Lista de blocos: cada bloco (card/contexto) com seus terminais, notas e card.
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
            Self::Blocks => "Cada contexto (card) é um bloco com seus terminais, notas, tarefas e card do Notion, um embaixo do outro.",
            Self::Tabs => "Um bloco por aba, terminais divididos lado a lado. O jeito tmux.",
            Self::Focus => "Um terminal grande e a lista de sessões ao lado, com prévia.",
            Self::Grid => "Todos os terminais do workspace numa grade.",
        }
    }
}

/// Por onde o AIC fala com Notion/Slack.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Connector {
    /// `claude -p` usando os conectores (Notion/Slack) já ligados na conta do usuário.
    #[default]
    Claude,
    /// API direta com token (mais rápido, precisa de NOTION_TOKEN no .env).
    Token,
}

/// Preferências por tema.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(default)]
pub struct ThemeConfig {
    pub variant: Option<String>,
    /// Módulos ligados/desligados só neste tema.
    pub modules: Flags,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Config {
    /// Tema atual (`aic` | `plankton`).
    pub theme: String,
    /// Primeiro uso concluído (assistente de visualização/tema/módulos).
    pub setup_done: bool,
    /// Visualização das sessões.
    pub view: SessionView,
    /// Módulos ligados/desligados para todos os temas.
    pub modules: Flags,
    pub themes: BTreeMap<String, ThemeConfig>,
    /// Tecla líder: aperte e veja o menu de atalhos (estilo prefixo do tmux).
    pub leader: String,
    pub font_family: String,
    pub font_size: f32,
    pub ui_scale: f32,
    /// Pastas onde procurar repos (subpastas com `.git`), pra `#repo` e pra abrir sessões.
    pub repos_dirs: Vec<PathBuf>,
    pub connector: Connector,
    pub sound: SoundConfig,
    pub notify: NotifyConfig,
    /// Statusline do usuário, chamada pelo wrapper do AIC depois de gravar o snapshot.
    pub user_statusline: Option<String>,
    /// Mascote animado e partículas.
    pub animations: bool,
    pub notion: NotionConfig,
    pub slack: SlackConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct SoundConfig {
    pub enabled: bool,
    /// Id do som de "terminou" no catálogo (ex: `aic-acorde`, `plankton-gemido`) ou caminho de arquivo.
    /// `None` = o som padrão do tema.
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

/// Integração com o Notion. Nada de id real no código: os ids vêm do `~/.config/aic/.env`
/// (veja `.env.example`) ou de Config → Integrações.
/// O token NÃO fica aqui: vai no `.env` / `secrets.toml` (600) ou na env `NOTION_TOKEN`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct NotionConfig {
    /// Database (ou data source) de tasks/cards. Aceita o id da database ou do data source.
    pub tasks_database: Option<String>,
    /// Database (ou data source) de sprints, pra descobrir a sprint atual.
    pub sprints_database: Option<String>,
    /// Propriedades usadas no board (nomes no Notion).
    /// Título: vazio = a propriedade do tipo `title`, seja qual for o nome.
    pub title_property: Option<String>,
    pub status_property: Option<String>,
    /// Propriedade `unique_id` (ID) que gera a chave do card (ex: `DEV-142`).
    pub key_property: Option<String>,
    /// Prefixo usado quando o `unique_id` não tem prefixo próprio no Notion.
    pub key_prefix: Option<String>,
    pub assignee_property: Option<String>,
    /// Relação da task com a database de sprints.
    pub sprint_property: Option<String>,
    pub priority_property: Option<String>,
    /// Select com o tipo do card (Story, Subtask, BUG…); vira tag no AIC.
    pub type_property: Option<String>,
    /// Status da sprint na database de sprints e o valor que significa "atual".
    pub sprint_status_property: Option<String>,
    pub sprint_current_value: Option<String>,
    /// Filtra "meus cards" por este usuário (e-mail ou id do Notion).
    pub me: Option<String>,
    /// Só cards em que `me` é responsável.
    pub only_mine: bool,
    /// Só cards da sprint atual.
    pub only_current_sprint: bool,
    /// Intervalo do sync automático (0 = só manual / ao abrir o board).
    pub sync_minutes: u32,
    /// Nomes de status do Notion por coluna. O primeiro de cada lista é o que o AIC grava.
    pub status_map: NotionStatusMap,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct NotionStatusMap {
    pub todo: Vec<String>,
    pub doing: Vec<String>,
    pub review: Vec<String>,
    pub done: Vec<String>,
    pub archived: Vec<String>,
}

fn strs(v: &[&str]) -> Vec<String> {
    v.iter().map(|s| s.to_string()).collect()
}

impl Default for NotionStatusMap {
    fn default() -> Self {
        Self {
            todo: strs(&["Não iniciado", "A fazer", "Backlog", "To do", "Not started"]),
            doing: strs(&["Em andamento", "Fazendo", "In progress"]),
            review: strs(&["Em revisão", "Code review", "Review", "QA"]),
            done: strs(&["Concluído", "Feito", "Done"]),
            archived: strs(&["Arquivado", "Archived"]),
        }
    }
}

impl Default for NotionConfig {
    fn default() -> Self {
        Self {
            tasks_database: None,
            sprints_database: None,
            title_property: None,
            status_property: Some("Status".into()),
            key_property: Some("ID".into()),
            key_prefix: None,
            assignee_property: Some("Responsável".into()),
            sprint_property: Some("Sprint".into()),
            priority_property: Some("Prioridade".into()),
            type_property: Some("Tipo".into()),
            sprint_status_property: Some("Sprint status".into()),
            sprint_current_value: Some("Current".into()),
            me: None,
            only_mine: true,
            only_current_sprint: true,
            sync_minutes: 10,
            status_map: NotionStatusMap::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(default)]
pub struct SlackConfig {
    pub workspace_url: Option<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            theme: "aic".into(),
            setup_done: false,
            view: SessionView::Blocks,
            modules: Flags::new(),
            themes: BTreeMap::new(),
            leader: "Ctrl+Space".into(),
            font_family: "JetBrainsMono Nerd Font Mono".into(),
            font_size: 14.0,
            ui_scale: 1.0,
            repos_dirs: vec![paths::home_dir()],
            connector: Connector::Claude,
            sound: SoundConfig::default(),
            notify: NotifyConfig::default(),
            user_statusline: None,
            animations: true,
            notion: NotionConfig::default(),
            slack: SlackConfig::default(),
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
        if let Some(v) = env("AIC_NOTION_TASKS_DB") {
            self.notion.tasks_database = Some(v);
        }
        if let Some(v) = env("AIC_NOTION_SPRINTS_DB") {
            self.notion.sprints_database = Some(v);
        }
        if let Some(v) = env("AIC_NOTION_ME") {
            self.notion.me = Some(v);
        }
        if let Some(v) = env("AIC_NOTION_KEY_PREFIX") {
            self.notion.key_prefix = Some(v);
        }
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
        if let Some(v) = env("SLACK_WORKSPACE_URL") {
            self.slack.workspace_url = Some(v);
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

    /// Módulo ligado no tema atual?
    pub fn on(&self, m: Module) -> bool {
        modules::enabled(m, &self.modules, self.themes.get(&self.theme).map(|t| &t.modules))
    }

    pub fn theme_cfg(&mut self) -> &mut ThemeConfig {
        self.themes.entry(self.theme.clone()).or_default()
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
