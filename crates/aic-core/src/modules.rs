//! Feature flags. Cada módulo liga/desliga em `[modules]` no config.toml (ou em Config → Módulos).
//!
//! Desligar um módulo some com a aba, os atalhos e as ações dele.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Module {
    /// Tarefas locais: captura rápida e o painel Tarefas do bloco.
    Tasks,
    /// Notas por sessão.
    Notes,
    /// Aba de worktrees e worktree por sessão.
    Worktrees,
    /// Aba Changes: arquivos mudados e diff do repo, como o "Source Control" do VS Code.
    Changes,
    /// Aba de tokens, limites e histórico de custo.
    Usage,
    /// Terminais sobre tmux: sobrevivem ao fechar o app.
    Tmux,
    /// Vários workspaces.
    Workspaces,
    /// Som quando uma sessão termina / precisa de você.
    Sound,
    /// Notificação do sistema.
    Notifications,
    /// Catálogo de atalhos (tecla líder).
    Hints,
    /// Layne: assistente pessoal (um Claude sempre aberto, por texto ou voz) que mexe no AIC por você.
    Layne,
}

pub struct ModuleInfo {
    pub module: Module,
    pub id: &'static str,
    pub name: &'static str,
    pub desc: &'static str,
    pub group: &'static str,
    pub default: bool,
    /// Só funciona com estes ligados.
    pub requires: &'static [Module],
}

use Module::*;

pub const ALL: &[ModuleInfo] = &[
    ModuleInfo {
        module: Tmux,
        id: "tmux",
        name: "Persistência (tmux)",
        desc: "Terminais rodam num servidor tmux próprio do AIC: fechar a janela não mata nada, e ao abrir ele reconecta.",
        group: "Sessões",
        default: true,
        requires: &[],
    },
    ModuleInfo {
        module: Workspaces,
        id: "workspaces",
        name: "Workspaces",
        desc: "Vários contextos de trabalho, cada um com seus blocos e layout.",
        group: "Sessões",
        default: true,
        requires: &[],
    },
    ModuleInfo {
        module: Layne,
        id: "layne",
        name: "Layne (assistente)",
        desc: "Um Claude sempre aberto no painel lateral: fala por texto ou voz e ele cria workspaces, tarefas e sessões por você.",
        group: "Sessões",
        default: true,
        requires: &[],
    },
    ModuleInfo {
        module: Tasks,
        id: "tasks",
        name: "Tarefas",
        desc: "Tarefas locais: captura rápida com marcadores (#repo @hoje !alta) e o painel Tarefas do bloco.",
        group: "Organização",
        default: true,
        requires: &[],
    },
    ModuleInfo {
        module: Notes,
        id: "notes",
        name: "Notas",
        desc: "Anotações em Markdown por sessão, num painel lateral (Ctrl+B e), salvas sozinhas.",
        group: "Organização",
        default: true,
        requires: &[],
    },
    ModuleInfo {
        module: Worktrees,
        id: "worktrees",
        name: "Worktrees",
        desc: "Aba de worktrees e opção de isolar cada sessão num worktree.",
        group: "Organização",
        default: true,
        requires: &[],
    },
    ModuleInfo {
        module: Changes,
        id: "changes",
        name: "Changes",
        desc: "Aba com os arquivos mudados e o diff de cada um (como o Source Control do VS Code): stage, descartar e commit.",
        group: "Organização",
        default: true,
        requires: &[],
    },
    ModuleInfo {
        module: Usage,
        id: "usage",
        name: "Tokens e custo",
        desc: "Limites de 5h/semana e histórico de custo lido dos transcripts.",
        group: "Organização",
        default: true,
        requires: &[],
    },
    ModuleInfo {
        module: Hints,
        id: "hints",
        name: "Catálogo de atalhos",
        desc: "Tecla líder (Ctrl+Space) com todos os atalhos agrupados.",
        group: "Interface",
        default: true,
        requires: &[],
    },
    ModuleInfo {
        module: Sound,
        id: "sound",
        name: "Som",
        desc: "Toca um som quando uma sessão termina ou pede permissão.",
        group: "Avisos",
        default: true,
        requires: &[],
    },
    ModuleInfo {
        module: Notifications,
        id: "notifications",
        name: "Notificações",
        desc: "Notificação do sistema quando uma sessão termina ou precisa de você.",
        group: "Avisos",
        default: true,
        requires: &[],
    },
];

pub fn info(m: Module) -> &'static ModuleInfo {
    ALL.iter()
        .find(|i| i.module == m)
        .expect("módulo registrado")
}

pub fn by_id(id: &str) -> Option<Module> {
    ALL.iter().find(|i| i.id == id).map(|i| i.module)
}

/// Overrides por id (`"tasks" = false`).
pub type Flags = BTreeMap<String, bool>;

/// Ligado? O que está no config ou o padrão, e só se as dependências também estiverem.
pub fn enabled(m: Module, flags: &Flags) -> bool {
    let i = info(m);
    let own = flags.get(i.id).copied().unwrap_or(i.default);
    own && i.requires.iter().all(|r| enabled(*r, flags))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn padrao_e_override() {
        let mut g = Flags::new();
        assert!(enabled(Module::Tasks, &g));
        g.insert("tasks".into(), false);
        assert!(!enabled(Module::Tasks, &g));
    }

    #[test]
    fn ids_unicos_e_resolvem() {
        let mut seen = std::collections::HashSet::new();
        for i in ALL {
            assert!(seen.insert(i.id));
            assert_eq!(by_id(i.id), Some(i.module));
        }
    }
}
