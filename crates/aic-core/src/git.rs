//! Git via CLI: branch, sujeira e worktrees.
//!
//! Worktrees seguem o padrão `<repo>.worktrees/<branch>` ao lado do
//! repo. Remover um worktree nunca apaga a branch.

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail};

fn git(dir: &Path, args: &[&str]) -> Result<String> {
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .env("GIT_TERMINAL_PROMPT", "0")
        .output()
        .context("rodando git")?;
    if !out.status.success() {
        bail!("git {}: {}", args.join(" "), String::from_utf8_lossy(&out.stderr).trim());
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim_end().to_string())
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct GitStatus {
    pub branch: Option<String>,
    /// Arquivos modificados/novos (o `±8` da statusline).
    pub dirty: usize,
    pub ahead: usize,
    pub behind: usize,
}

pub fn status(dir: &Path) -> Option<GitStatus> {
    let out = git(dir, &["--no-optional-locks", "status", "--porcelain=v2", "--branch"]).ok()?;
    let mut st = GitStatus::default();
    for line in out.lines() {
        if let Some(b) = line.strip_prefix("# branch.head ") {
            st.branch = (b != "(detached)").then(|| b.to_string());
        } else if let Some(ab) = line.strip_prefix("# branch.ab ") {
            for part in ab.split_whitespace() {
                if let Some(n) = part.strip_prefix('+') {
                    st.ahead = n.parse().unwrap_or(0);
                } else if let Some(n) = part.strip_prefix('-') {
                    st.behind = n.parse().unwrap_or(0);
                }
            }
        } else if !line.starts_with('#') && !line.is_empty() {
            st.dirty += 1;
        }
    }
    Some(st)
}

/// Raiz do repositório principal (mesmo de dentro de um worktree).
pub fn main_root(dir: &Path) -> Option<PathBuf> {
    let common = git(dir, &["rev-parse", "--path-format=absolute", "--git-common-dir"]).ok()?;
    let common = PathBuf::from(common);
    if common.file_name().is_some_and(|n| n == ".git") {
        common.parent().map(Path::to_path_buf)
    } else {
        Some(common)
    }
}

pub fn toplevel(dir: &Path) -> Option<PathBuf> {
    git(dir, &["rev-parse", "--show-toplevel"]).ok().map(PathBuf::from)
}

#[derive(Debug, Clone, PartialEq)]
pub struct Worktree {
    pub path: PathBuf,
    pub branch: Option<String>,
    pub head: String,
    pub is_main: bool,
    pub locked: bool,
    pub prunable: bool,
}

pub fn worktrees(repo: &Path) -> Result<Vec<Worktree>> {
    let out = git(repo, &["worktree", "list", "--porcelain"])?;
    let mut list = Vec::new();
    let mut cur: Option<Worktree> = None;
    for line in out.lines().chain(std::iter::once("")) {
        if let Some(p) = line.strip_prefix("worktree ") {
            cur = Some(Worktree {
                path: PathBuf::from(p),
                branch: None,
                head: String::new(),
                is_main: list.is_empty(),
                locked: false,
                prunable: false,
            });
        } else if let Some(w) = cur.as_mut() {
            if let Some(h) = line.strip_prefix("HEAD ") {
                w.head = h.chars().take(8).collect();
            } else if let Some(b) = line.strip_prefix("branch ") {
                w.branch = Some(b.trim_start_matches("refs/heads/").to_string());
            } else if line.starts_with("locked") {
                w.locked = true;
            } else if line.starts_with("prunable") {
                w.prunable = true;
            } else if line.is_empty() {
                list.push(cur.take().unwrap());
            }
        }
    }
    Ok(list)
}

/// Branch padrão do repo, pela branch do origin, senão `main`/`master`.
pub fn default_branch(repo: &Path) -> String {
    if let Ok(r) = git(repo, &["symbolic-ref", "--short", "refs/remotes/origin/HEAD"]) {
        if let Some(b) = r.strip_prefix("origin/") {
            return b.to_string();
        }
    }
    for b in ["main", "master"] {
        if git(repo, &["rev-parse", "--verify", "--quiet", b]).is_ok() {
            return b.to_string();
        }
    }
    "main".into()
}

pub fn worktree_path(repo: &Path, branch: &str) -> PathBuf {
    let name = repo.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    let safe = branch.replace('/', "-");
    repo.with_file_name(format!("{name}.worktrees")).join(safe)
}

/// Cria (ou reaproveita) um worktree para `branch`. Se a branch não existe, sai de `base`.
pub fn add_worktree(repo: &Path, branch: &str, base: Option<&str>) -> Result<PathBuf> {
    let path = worktree_path(repo, branch);
    if path.exists() {
        return Ok(path);
    }
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let p = path.to_string_lossy().to_string();
    let exists = git(repo, &["rev-parse", "--verify", "--quiet", &format!("refs/heads/{branch}")]).is_ok();
    if exists {
        git(repo, &["worktree", "add", &p, branch])?;
    } else {
        let base = base.map(String::from).unwrap_or_else(|| default_branch(repo));
        git(repo, &["worktree", "add", "-b", branch, &p, &base])?;
    }
    Ok(path)
}

/// Remove o worktree (a branch fica). `force` descarta mudanças não commitadas.
pub fn remove_worktree(repo: &Path, path: &Path, force: bool) -> Result<()> {
    let p = path.to_string_lossy().to_string();
    let mut args = vec!["worktree", "remove"];
    if force {
        args.push("--force");
    }
    args.push(&p);
    git(repo, &args)?;
    Ok(())
}

pub fn prune(repo: &Path) -> Result<()> {
    git(repo, &["worktree", "prune"])?;
    Ok(())
}

/// Nome de branch a partir de um card/tarefa: `DEV-142` + "corrigir juros" → `DEV-142-corrigir-juros`.
pub fn branch_slug(card: Option<&str>, title: &str) -> String {
    let mut slug: String = title
        .to_lowercase()
        .chars()
        .map(|c| match c {
            'á' | 'à' | 'â' | 'ã' => 'a',
            'é' | 'ê' => 'e',
            'í' => 'i',
            'ó' | 'ô' | 'õ' => 'o',
            'ú' => 'u',
            'ç' => 'c',
            c if c.is_ascii_alphanumeric() => c,
            _ => '-',
        })
        .collect();
    while slug.contains("--") {
        slug = slug.replace("--", "-");
    }
    let slug: String = slug.trim_matches('-').chars().take(40).collect();
    let slug = slug.trim_end_matches('-');
    match card {
        Some(c) if !c.is_empty() => format!("{c}-{slug}"),
        _ => slug.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slug() {
        assert_eq!(branch_slug(Some("DEV-142"), "Corrigir cálculo de juros!"), "DEV-142-corrigir-calculo-de-juros");
        assert_eq!(branch_slug(None, "  ajuste / canvas  "), "ajuste-canvas");
    }

    #[test]
    fn caminho_do_worktree() {
        let p = worktree_path(Path::new("/x/repos/api-pagamentos"), "feat/abc");
        assert_eq!(p, PathBuf::from("/x/repos/api-pagamentos.worktrees/feat-abc"));
    }

    #[test]
    fn worktree_de_ponta_a_ponta() {
        let dir = std::env::temp_dir().join(format!("aic-git-{}", uuid::Uuid::new_v4()));
        let repo = dir.join("repo");
        std::fs::create_dir_all(&repo).unwrap();
        let run = |args: &[&str]| git(&repo, args).unwrap();
        run(&["init", "-q", "-b", "main"]);
        run(&["-c", "user.email=t@t", "-c", "user.name=t", "-c", "commit.gpgsign=false", "commit", "-q", "--allow-empty", "-m", "init"]);
        let wt = add_worktree(&repo, "DEV-1-teste", None).unwrap();
        assert!(wt.exists());
        let list = worktrees(&repo).unwrap();
        assert_eq!(list.len(), 2);
        assert_eq!(list[1].branch.as_deref(), Some("DEV-1-teste"));
        assert_eq!(main_root(&wt).unwrap().canonicalize().unwrap(), repo.canonicalize().unwrap());
        std::fs::write(wt.join("a.txt"), "x").unwrap();
        assert_eq!(status(&wt).unwrap().dirty, 1);
        remove_worktree(&repo, &wt, true).unwrap();
        assert_eq!(worktrees(&repo).unwrap().len(), 1);
        // branch continua existindo
        assert!(git(&repo, &["rev-parse", "--verify", "DEV-1-teste"]).is_ok());
        let _ = std::fs::remove_dir_all(dir);
    }
}
