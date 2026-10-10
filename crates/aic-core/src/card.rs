//! "Abrir card": buscar um card do Notion (e as threads do Slack que ele cita) e planejar o
//! bloco de trabalho em cima dele.
//!
//! Duas pontes, escolhidas por `Config::connector`:
//! - `Connector::Claude`: `claude -p` usando os conectores MCP já ligados na conta (claude.ai
//!   Notion/Slack). O prompt vai por stdin e só ferramentas de LEITURA são liberadas com
//!   `--allowedTools` — no modo não-interativo, ferramenta fora da lista é negada.
//! - `Connector::Token`: API do Notion direto, com o token (sem Slack).
//!
//! As buscas são bloqueantes: chame de uma thread de fundo. O resto é função pura.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::config::{Connector, NotionConfig};
use crate::git;
use crate::notion::{self, Client, NotionCard};
use crate::paths;

/// Quanto esperar o `claude -p` antes de desistir.
pub const FETCH_TIMEOUT: Duration = Duration::from_secs(120);

/// Ferramentas que o `claude -p` pode usar na busca: só leitura.
/// Os nomes dos conectores do claude.ai e dos plugins diferem; os ausentes são ignorados.
pub const READ_TOOLS: &[&str] = &[
    "ToolSearch",
    "mcp__claude_ai_Notion__notion-fetch",
    "mcp__claude_ai_Notion__notion-search",
    "mcp__claude_ai_Notion__notion-get-comments",
    "mcp__claude_ai_Notion__notion-get-users",
    "mcp__claude_ai_Slack__slack_read_thread",
    "mcp__claude_ai_Slack__slack_search_public_and_private",
    "mcp__plugin_product-management_notion__notion-fetch",
    "mcp__plugin_product-management_notion__notion-search",
    "mcp__plugin_slack-by-salesforce_slack__slack_read_thread",
];

/// Variáveis de uma sessão do Claude Code que não devem vazar pro `claude -p` filho.
const CLAUDE_ENV: &[&str] = &[
    "CLAUDECODE",
    "CLAUDE_CODE_ENTRYPOINT",
    "CLAUDE_CODE_CHILD_SESSION",
    "CLAUDE_CODE_SESSION_ID",
    "CLAUDE_CODE_SESSION_ATTENDED",
    "CLAUDE_CODE_MESSAGING_SOCKET",
    "CLAUDE_CODE_MESSAGING_TOKEN",
    "CLAUDE_PID",
];

/// Um card já lido, no formato que o AIC usa (e que fica em cache no `kv`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CardInfo {
    pub title: String,
    /// Chave do card (ex: `DEV-123`).
    pub key: Option<String>,
    pub url: Option<String>,
    /// Status como está no Notion.
    pub status: Option<String>,
    pub assignee: Option<String>,
    /// Descrição resumida (algumas frases).
    pub summary: String,
    /// Critérios de aceite, um por item.
    pub criteria: Vec<String>,
    /// Links de threads do Slack citadas no card.
    pub slack_threads: Vec<String>,
    /// Repos prováveis (nomes da lista conhecida).
    pub repos: Vec<String>,
}

impl CardInfo {
    /// Page id do Notion, tirado da URL.
    pub fn page_id(&self) -> Option<String> {
        self.url.as_deref().and_then(notion::page_id_from)
    }

    /// Texto do painel "resumo do card".
    pub fn summary_text(&self) -> String {
        let mut s = String::new();
        let meta: Vec<&str> = [self.status.as_deref(), self.assignee.as_deref()].into_iter().flatten().collect();
        if !meta.is_empty() {
            s.push_str(&meta.join(" · "));
            s.push_str("\n\n");
        }
        s.push_str(self.summary.trim());
        if !self.criteria.is_empty() {
            s.push_str("\n\nCritérios:");
            for c in &self.criteria {
                s.push_str(&format!("\n• {c}"));
            }
        }
        s.trim().to_string()
    }
}

// ───────────────────────────── referência ─────────────────────────────

/// O que o usuário colou.
#[derive(Debug, Clone, PartialEq)]
pub enum CardRef {
    /// Link ou id de página do Notion (id normalizado).
    Page(String),
    /// Chave tipo `DEV-123`.
    Key { prefix: String, number: i64 },
    Unknown,
}

pub fn card_ref(input: &str) -> CardRef {
    let s = input.trim();
    if let Some((prefix, number)) = parse_key(s) {
        return CardRef::Key { prefix, number };
    }
    match notion::page_id_from(s) {
        Some(id) => CardRef::Page(id),
        None => CardRef::Unknown,
    }
}

/// `dev-123` → (`DEV`, 123).
fn parse_key(s: &str) -> Option<(String, i64)> {
    let (prefix, n) = s.rsplit_once('-')?;
    let ok = !prefix.is_empty() && prefix.len() <= 12 && prefix.chars().all(|c| c.is_ascii_alphabetic());
    let number = n.parse::<i64>().ok().filter(|_| ok && n.chars().all(|c| c.is_ascii_digit()))?;
    Some((prefix.to_ascii_uppercase(), number))
}

/// Rótulo curto pra mensagens ("DEV-123", "a página do Notion").
pub fn ref_label(input: &str) -> String {
    match card_ref(input) {
        CardRef::Key { prefix, number } => format!("{prefix}-{number}"),
        CardRef::Page(_) => "o card".into(),
        CardRef::Unknown => format!("\"{}\"", truncate(input.trim(), 30)),
    }
}

// ───────────────────────────── prompt e parse ─────────────────────────────

/// Prompt da busca via `claude -p`. Pede SÓ um JSON, com os repos escolhidos de `repos`.
pub fn build_prompt(input: &str, repos: &[String], slack: bool) -> String {
    let slack = if slack {
        " Se o card citar threads do Slack, leia-as com as ferramentas do Slack para entender o contexto."
    } else {
        " Não use o Slack."
    };
    let list = if repos.is_empty() { "(nenhum configurado)".to_string() } else { repos.iter().map(|r| format!("- {r}")).collect::<Vec<_>>().join("\n") };
    format!(
        r#"Você é um extrator de dados. Use as ferramentas MCP do Notion disponíveis nesta sessão (se não aparecerem, procure com ToolSearch por "notion") para LER o card abaixo.{slack} SOMENTE LEITURA: não crie, edite, comente nem envie nada.

Card: {input}

Repositórios disponíveis (escolha de 0 a 3 que provavelmente vão mudar, usando exatamente estes nomes):
{list}

Responda APENAS com um objeto JSON, sem texto antes ou depois e sem cercas de código, neste formato:
{{"titulo": "título do card", "chave": "DEV-123 ou null", "url": "link do card no Notion", "status": "status no Notion", "responsavel": "nome da pessoa ou null (nunca um id)", "resumo": "2 a 5 frases sobre o que precisa ser feito e por quê (inclua o que as threads do Slack decidiram)", "criterios": ["critério de aceite"], "slack": ["links das threads do Slack citadas"], "repos": ["nome-do-repo"]}}

Se não conseguir acessar o card, responda {{"erro": "motivo curto"}}."#
    )
}

/// Argumentos do `claude` pra busca (o prompt vai por stdin).
pub fn claude_args(model: Option<&str>) -> Vec<String> {
    let mut a: Vec<String> = ["-p", "--output-format", "json"].iter().map(|s| s.to_string()).collect();
    if let Some(m) = model.filter(|m| !m.is_empty()) {
        a.extend(["--model".into(), m.to_string()]);
    }
    a.push("--allowedTools".into());
    a.extend(READ_TOOLS.iter().map(|s| s.to_string()));
    a
}

/// Primeiro objeto JSON dentro de um texto (com prosa ou cercas ``` em volta).
/// Prefere objetos com cara de card (`titulo`/`title`/`erro`).
pub fn extract_json(text: &str) -> Option<Value> {
    let mut first: Option<Value> = None;
    for (start, _) in text.match_indices('{') {
        let Some(end) = balanced_end(&text[start..]) else { continue };
        let Ok(v) = serde_json::from_str::<Value>(&text[start..start + end]) else { continue };
        if !v.is_object() {
            continue;
        }
        if ["titulo", "title", "erro", "error"].iter().any(|k| v.get(k).is_some()) {
            return Some(v);
        }
        first.get_or_insert(v);
    }
    first
}

/// Fim (exclusivo) do objeto que começa em `s[0] == '{'`, respeitando strings.
fn balanced_end(s: &str) -> Option<usize> {
    let (mut depth, mut in_str, mut esc) = (0i32, false, false);
    for (i, c) in s.char_indices() {
        if in_str {
            match c {
                _ if esc => esc = false,
                '\\' => esc = true,
                '"' => in_str = false,
                _ => {}
            }
            continue;
        }
        match c {
            '"' => in_str = true,
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(i + 1);
                }
            }
            _ => {}
        }
    }
    None
}

fn str_of(v: &Value, keys: &[&str]) -> Option<String> {
    keys.iter().find_map(|k| match v.get(k)? {
        Value::String(s) => Some(s.trim().to_string()),
        Value::Number(n) => Some(n.to_string()),
        _ => None,
    })
    .filter(|s| !s.is_empty() && s != "null")
}

fn list_of(v: &Value, keys: &[&str]) -> Vec<String> {
    let Some(x) = keys.iter().find_map(|k| v.get(k)) else { return vec![] };
    let items = match x {
        Value::Array(a) => a.iter().filter_map(|i| i.as_str().map(str::to_string)).collect(),
        Value::String(s) => vec![s.clone()],
        _ => vec![],
    };
    items.into_iter().map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect()
}

/// Objeto JSON do Claude → card. Aceita as chaves em português ou inglês e valores nulos.
pub fn card_from_value(v: &Value) -> CardInfo {
    CardInfo {
        title: str_of(v, &["titulo", "title"]).unwrap_or_default(),
        key: str_of(v, &["chave", "key"]),
        url: str_of(v, &["url"]),
        status: str_of(v, &["status"]),
        assignee: str_of(v, &["responsavel", "assignee"]),
        summary: str_of(v, &["resumo", "summary", "descricao"]).unwrap_or_default(),
        criteria: list_of(v, &["criterios", "criteria"]),
        slack_threads: list_of(v, &["slack", "slack_threads"]),
        repos: list_of(v, &["repos"]),
    }
}

/// Texto da resposta do Claude → card (ou erro legível).
pub fn parse_card_text(text: &str) -> Result<CardInfo, String> {
    let v = extract_json(text).ok_or_else(|| format!("o Claude respondeu sem um JSON válido: \"{}\"", truncate(text.trim(), 160)))?;
    if let Some(e) = str_of(&v, &["erro", "error"]) {
        return Err(format!("o Claude não conseguiu ler o card: {e}"));
    }
    let card = card_from_value(&v);
    if card.title.is_empty() {
        return Err("a resposta do Claude veio sem título".into());
    }
    Ok(card)
}

/// Saída do `claude -p --output-format json` → card. `result` vem como string.
pub fn parse_claude_output(stdout: &str) -> Result<CardInfo, String> {
    let outer = serde_json::from_str::<Value>(stdout.trim()).ok().filter(|v| v.get("result").is_some() || v.get("is_error").is_some());
    let Some(outer) = outer else { return parse_card_text(stdout) };
    let result = outer.get("result").and_then(Value::as_str).unwrap_or_default();
    if outer.get("is_error").and_then(Value::as_bool).unwrap_or(false) {
        let msg = if result.is_empty() { outer.get("subtype").and_then(Value::as_str).unwrap_or("erro").to_string() } else { truncate(result, 200) };
        return Err(format!("o Claude devolveu erro: {msg}"));
    }
    parse_card_text(result)
}

/// Mantém só os repos conhecidos (com a grafia da lista), sem repetir, no máximo 3.
pub fn normalize_repos(wanted: &[String], known: &[String]) -> Vec<String> {
    let mut out: Vec<String> = vec![];
    for w in wanted {
        let w = w.trim().trim_start_matches('#').to_lowercase();
        if let Some(k) = known.iter().find(|k| k.to_lowercase() == w)
            && !out.contains(k)
        {
            out.push(k.clone());
        }
    }
    out.truncate(3);
    out
}

/// Repos citados pelo nome (palavra inteira) num texto (modo token, sem IA).
pub fn guess_repos(text: &str, known: &[String]) -> Vec<String> {
    let words: Vec<String> = text
        .to_lowercase()
        .split(|c: char| !(c.is_alphanumeric() || matches!(c, '-' | '_' | '.')))
        .map(|w| w.trim_matches(['.', '-']).to_string())
        .collect();
    let hits: Vec<String> = words.iter().filter(|w| known.iter().any(|k| k.to_lowercase() == **w)).cloned().collect();
    normalize_repos(&hits, known)
}

/// Links de threads do Slack num texto.
pub fn slack_links(text: &str) -> Vec<String> {
    let mut v: Vec<String> = text
        .split(|c: char| c.is_whitespace() || matches!(c, '(' | ')' | '<' | '>' | '"' | '\''))
        .filter(|w| w.contains("slack.com/archives/"))
        .map(|w| w.trim_end_matches(['.', ',', ';']).to_string())
        .collect();
    v.dedup();
    v
}

// ───────────────────────────── busca ─────────────────────────────

/// Busca o card pela ponte configurada. `cancel` interrompe (mata o `claude -p`).
pub fn fetch(connector: Connector, input: &str, repos: &[String], notion_cfg: &NotionConfig, slack: bool, cancel: &AtomicBool) -> Result<CardInfo, String> {
    let mut card = match connector {
        Connector::Claude => fetch_with_claude(input, repos, slack, FETCH_TIMEOUT, cancel)?,
        Connector::Token => fetch_with_token(input, notion_cfg, repos)?,
    };
    card.repos = normalize_repos(&card.repos, repos);
    if card.key.is_none()
        && let CardRef::Key { prefix, number } = card_ref(input)
    {
        card.key = Some(format!("{prefix}-{number}"));
    }
    if card.url.is_none() && input.trim().starts_with("http") {
        card.url = Some(input.trim().to_string());
    }
    Ok(card)
}

pub fn fetch_with_claude(input: &str, repos: &[String], slack: bool, timeout: Duration, cancel: &AtomicBool) -> Result<CardInfo, String> {
    let out = run_claude(&claude_args(Some("haiku")), &build_prompt(input, repos, slack), timeout, cancel)?;
    parse_claude_output(&out)
}

/// Roda `claude` com o prompt no stdin; devolve o stdout. Mata no timeout ou no cancelamento.
fn run_claude(args: &[String], prompt: &str, timeout: Duration, cancel: &AtomicBool) -> Result<String, String> {
    let mut cmd = Command::new("claude");
    cmd.args(args).current_dir(paths::home_dir()).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
    for v in CLAUDE_ENV {
        cmd.env_remove(v);
    }
    let mut child = cmd.spawn().map_err(|e| match e.kind() {
        std::io::ErrorKind::NotFound => "Claude Code (`claude`) não está no PATH. Instale, ou troque o conector para Token em Config → Integrações.".to_string(),
        _ => format!("não deu pra rodar o claude: {e}"),
    })?;
    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(prompt.as_bytes());
    }
    let pipe = |r: Option<Box<dyn Read + Send>>| {
        std::thread::spawn(move || {
            let mut s = String::new();
            if let Some(mut r) = r {
                let _ = r.read_to_string(&mut s);
            }
            s
        })
    };
    let out = pipe(child.stdout.take().map(|r| Box::new(r) as Box<dyn Read + Send>));
    let err = pipe(child.stderr.take().map(|r| Box::new(r) as Box<dyn Read + Send>));
    let start = Instant::now();
    let status = loop {
        if let Ok(Some(st)) = child.try_wait() {
            break st;
        }
        if cancel.load(Ordering::Relaxed) {
            let _ = child.kill();
            let _ = child.wait();
            return Err("busca cancelada".into());
        }
        if start.elapsed() > timeout {
            let _ = child.kill();
            let _ = child.wait();
            return Err(format!("o Claude não respondeu em {}s (Notion lento ou conector desconectado — veja `claude mcp list`)", timeout.as_secs()));
        }
        std::thread::sleep(Duration::from_millis(100));
    };
    let stdout = out.join().unwrap_or_default();
    let stderr = err.join().unwrap_or_default();
    if stdout.trim().is_empty() {
        let tail: String = stderr.trim().lines().last().unwrap_or("sem saída").to_string();
        return Err(format!("o claude saiu com {status}: {}", truncate(&tail, 200)));
    }
    Ok(stdout)
}

/// Modo token: API do Notion (link/id da página ou chave `DEV-123` na database de tasks).
pub fn fetch_with_token(input: &str, cfg: &NotionConfig, repos: &[String]) -> Result<CardInfo, String> {
    let client = Client::from_saved().map_err(|e| format!("{e:#}"))?;
    let page = match card_ref(input) {
        CardRef::Page(id) => client.page(&id).map_err(|e| format!("{e:#}"))?,
        CardRef::Key { number, .. } => {
            let db = cfg.tasks_database.as_deref().filter(|s| !s.trim().is_empty()).ok_or("buscar por chave precisa da database de tasks (Config → Integrações)")?;
            let ds = client.data_source(db).map_err(|e| format!("{e:#}"))?;
            let ds_id = ds.get("id").and_then(Value::as_str).ok_or("data source sem id")?.to_string();
            let prop = cfg.key_property.clone().filter(|s| !s.trim().is_empty()).unwrap_or_else(|| "ID".into());
            let filter = json!({"property": prop, "unique_id": {"equals": number}});
            client.query_all(&ds_id, Some(filter)).map_err(|e| format!("{e:#}"))?.into_iter().next().ok_or_else(|| format!("nenhum card com a chave {}", input.trim()))?
        }
        CardRef::Unknown => return Err("cole um link do Notion ou uma chave tipo DEV-123".into()),
    };
    let card = notion::page_to_card(&page, cfg).ok_or("o link não é de uma página do Notion")?;
    let text = client.page_text(&card.page_id, 60).unwrap_or_default();
    Ok(card_from_page(&card, &text, repos))
}

/// Card do Notion (propriedades + texto da página) → `CardInfo`, sem IA.
pub fn card_from_page(card: &NotionCard, text: &str, repos: &[String]) -> CardInfo {
    CardInfo {
        title: card.title.clone(),
        key: card.key.clone(),
        url: card.url.clone(),
        status: card.status_name.clone(),
        assignee: None,
        summary: truncate(text.trim(), 700),
        criteria: vec![],
        slack_threads: slack_links(text),
        repos: guess_repos(&format!("{}\n{text}", card.title), repos),
    }
}

// ───────────────────────────── plano de criação ─────────────────────────────

/// Escolhas do diálogo "Abrir card".
#[derive(Debug, Clone, PartialEq)]
pub struct CreateOptions {
    /// Um worktree por repo, na branch do card.
    pub worktree: bool,
    /// Um shell a mais (no primeiro repo).
    pub shell: bool,
    /// Notas do bloco com o resumo do card.
    pub notes: bool,
    /// Claude em `--permission-mode plan`.
    pub plan_mode: bool,
}

impl Default for CreateOptions {
    fn default() -> Self {
        Self { worktree: true, shell: false, notes: true, plan_mode: false }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlannedKind {
    Claude,
    Shell,
}

/// Um terminal a abrir.
#[derive(Debug, Clone, PartialEq)]
pub struct PlannedSession {
    pub kind: PlannedKind,
    pub name: String,
    pub repo: Option<String>,
    /// Pasta do repo (o worktree, se houver, é criado a partir dela).
    pub path: PathBuf,
    /// Branch do worktree a criar/reaproveitar. `None` = direto na pasta (ou no worktree já
    /// criado pra este repo, no caso do shell).
    pub branch: Option<String>,
    pub claude_args: Vec<String>,
    pub prompt: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CreationPlan {
    pub block_name: String,
    pub branch: String,
    pub sessions: Vec<PlannedSession>,
    /// Notas do bloco.
    pub notes: Option<String>,
    /// Conteúdo do `.aic/context.md` de cada pasta.
    pub context_md: String,
}

/// O que criar para um card: um Claude por repo (com worktree opcional), shell, notas.
/// Sem repo escolhido, um Claude em `fallback` (ex: home).
pub fn plan_creation(card: &CardInfo, repos: &[(String, PathBuf)], fallback: &Path, opts: &CreateOptions) -> CreationPlan {
    let branch = git::branch_slug(card.key.as_deref(), &card.title);
    let mut args = vec![];
    if opts.plan_mode {
        args.extend(["--permission-mode".to_string(), "plan".to_string()]);
    }
    let names: Vec<&str> = repos.iter().map(|(n, _)| n.as_str()).collect();
    let mut sessions: Vec<PlannedSession> = repos
        .iter()
        .map(|(name, path)| {
            let br = opts.worktree.then(|| branch.clone());
            PlannedSession {
                kind: PlannedKind::Claude,
                name: name.clone(),
                repo: Some(name.clone()),
                path: path.clone(),
                prompt: Some(initial_prompt(card, Some(name), br.as_deref(), &names)),
                branch: br,
                claude_args: args.clone(),
            }
        })
        .collect();
    if sessions.is_empty() {
        sessions.push(PlannedSession {
            kind: PlannedKind::Claude,
            name: card.key.clone().unwrap_or_else(|| "claude".into()),
            repo: None,
            path: fallback.to_path_buf(),
            branch: None,
            claude_args: args.clone(),
            prompt: Some(initial_prompt(card, None, None, &[])),
        });
    }
    if opts.shell {
        let first = sessions[0].clone();
        sessions.push(PlannedSession { kind: PlannedKind::Shell, name: "shell".into(), branch: None, claude_args: vec![], prompt: None, ..first });
    }
    CreationPlan { block_name: block_name(card), branch, sessions, notes: opts.notes.then(|| notes_md(card)), context_md: context_md(card) }
}

/// "DEV-123 · título curto".
pub fn block_name(card: &CardInfo) -> String {
    let short = truncate(card.title.trim(), 32);
    match &card.key {
        Some(k) => format!("{k} · {short}"),
        None => short,
    }
}

/// Primeira mensagem do Claude de um repo: contexto do card + pedir um plano antes de editar.
pub fn initial_prompt(card: &CardInfo, repo: Option<&str>, branch: Option<&str>, all_repos: &[&str]) -> String {
    let mut s = format!("Vamos trabalhar no card {}.\n", card.key.as_deref().map(|k| format!("{k}: {}", card.title)).unwrap_or_else(|| card.title.clone()));
    if let Some(u) = &card.url {
        s.push_str(&format!("Notion: {u}\n"));
    }
    if !card.summary.trim().is_empty() {
        s.push_str(&format!("\nResumo: {}\n", card.summary.trim()));
    }
    if !card.criteria.is_empty() {
        s.push_str("\nCritérios de aceite:\n");
        for c in &card.criteria {
            s.push_str(&format!("- {c}\n"));
        }
    }
    if !card.slack_threads.is_empty() {
        s.push_str(&format!("\nThreads do Slack: {}\n", card.slack_threads.join(" ")));
    }
    if let Some(r) = repo {
        s.push_str(&format!("\nVocê está no repo `{r}`"));
        if let Some(b) = branch {
            s.push_str(&format!(", num worktree na branch `{b}`"));
        }
        s.push('.');
        let others: Vec<&str> = all_repos.iter().copied().filter(|o| *o != r).collect();
        if !others.is_empty() {
            s.push_str(&format!(" O card também mexe em {} (cada um tem sua própria sessão); combine contratos (rotas, payloads) com eles.", others.join(", ")));
        }
        s.push('\n');
    }
    s.push_str("\nO contexto completo do card está em @.aic/context.md. Comece lendo o código relevante e me apresente um plano curto (arquivos a mudar, riscos, como testar) antes de editar qualquer coisa.");
    s
}

/// Notas do bloco: resumo, critérios como checklist, links.
pub fn notes_md(card: &CardInfo) -> String {
    let mut s = format!("## {}\n", block_name(card));
    if !card.summary.trim().is_empty() {
        s.push_str(&format!("\n{}\n", card.summary.trim()));
    }
    if !card.criteria.is_empty() {
        s.push_str("\n### Critérios\n");
        for c in &card.criteria {
            s.push_str(&format!("- [ ] {c}\n"));
        }
    }
    let links: Vec<String> = card.url.iter().map(|u| format!("- Notion: {u}")).chain(card.slack_threads.iter().map(|u| format!("- Slack: {u}"))).collect();
    if !links.is_empty() {
        s.push_str(&format!("\n### Links\n{}\n", links.join("\n")));
    }
    s
}

/// `.aic/context.md` escrito em cada pasta antes do Claude abrir.
pub fn context_md(card: &CardInfo) -> String {
    let mut s = format!("# {}\n\n", card.title);
    for (label, v) in [("Card", &card.key), ("Notion", &card.url), ("Status", &card.status), ("Responsável", &card.assignee)] {
        if let Some(v) = v {
            s.push_str(&format!("- {label}: {v}\n"));
        }
    }
    for u in &card.slack_threads {
        s.push_str(&format!("- Thread do Slack: {u}\n"));
    }
    if !card.repos.is_empty() {
        s.push_str(&format!("- Repos: {}\n", card.repos.join(", ")));
    }
    if !card.summary.trim().is_empty() {
        s.push_str(&format!("\n## Resumo\n\n{}\n", card.summary.trim()));
    }
    if !card.criteria.is_empty() {
        s.push_str("\n## Critérios de aceite\n\n");
        for c in &card.criteria {
            s.push_str(&format!("- [ ] {c}\n"));
        }
    }
    s.push_str("\n> Gerado pelo AIC (Abrir card). Para detalhes, leia o card no Notion pelo conector.\n");
    s
}

fn truncate(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        return s.to_string();
    }
    let mut t: String = s.chars().take(n.saturating_sub(1)).collect();
    t = t.trim_end().to_string();
    t.push('…');
    t
}

#[cfg(test)]
mod tests {
    use super::*;

    fn card() -> CardInfo {
        CardInfo {
            title: "Corrigir cálculo de juros no checkout".into(),
            key: Some("DEV-1256".into()),
            url: Some("https://www.notion.so/empresa/Corrigir-juros-0123456789abcdef0123456789abcdef".into()),
            status: Some("Em andamento".into()),
            assignee: Some("Fulano".into()),
            summary: "Juros diário erra centavos quando a proposta cruza o mês.".into(),
            criteria: vec!["arredondar no fim".into(), "teste de regressão".into()],
            slack_threads: vec!["https://x.slack.com/archives/C1/p1".into()],
            repos: vec!["api-pagamentos".into()],
        }
    }

    #[test]
    fn referencia_do_card() {
        assert_eq!(card_ref("dev-123"), CardRef::Key { prefix: "DEV".into(), number: 123 });
        assert_eq!(card_ref(" DEV-9 "), CardRef::Key { prefix: "DEV".into(), number: 9 });
        assert_eq!(card_ref("https://www.notion.so/x/Titulo-0123456789abcdef0123456789abcdef?pvs=4"), CardRef::Page("0123456789abcdef0123456789abcdef".into()));
        assert_eq!(card_ref("qualquer coisa"), CardRef::Unknown);
        assert_eq!(ref_label("dev-7"), "DEV-7");
    }

    #[test]
    fn json_com_texto_em_volta() {
        let text = "Claro! Aqui está:\n```json\n{\"titulo\": \"Tela {nova}\", \"chave\": \"DEV-1\", \"criterios\": null, \"repos\": [\"web-app\"], \"slack\": \"https://x.slack.com/archives/C1/p2\"}\n```\nQualquer coisa me avise {ok}.";
        let c = parse_card_text(text).unwrap();
        assert_eq!(c.title, "Tela {nova}");
        assert_eq!(c.key.as_deref(), Some("DEV-1"));
        assert!(c.criteria.is_empty());
        assert_eq!(c.repos, vec!["web-app"]);
        assert_eq!(c.slack_threads.len(), 1);
    }

    #[test]
    fn prefere_o_objeto_do_card() {
        let text = "Usei {\"x\": 1} e achei: {\"title\": \"A\", \"key\": null, \"url\": \"u\"}";
        let c = parse_card_text(text).unwrap();
        assert_eq!(c.title, "A");
        assert_eq!(c.key, None);
    }

    #[test]
    fn json_invalido_ou_erro() {
        assert!(parse_card_text("não achei nada").unwrap_err().contains("sem um JSON"));
        assert!(parse_card_text("{\"titulo\": \"abc\", ").is_err());
        assert!(parse_card_text("{\"erro\": \"sem acesso\"}").unwrap_err().contains("sem acesso"));
        assert!(parse_card_text("{\"titulo\": \"\"}").unwrap_err().contains("sem título"));
    }

    #[test]
    fn saida_do_claude_p() {
        let inner = json!({"titulo": "Juros", "chave": "DEV-2", "repos": ["API-Pagamentos", "nao-existe"]}).to_string();
        let out = json!({"type": "result", "is_error": false, "result": format!("Pronto:\n{inner}")}).to_string();
        let c = parse_claude_output(&out).unwrap();
        assert_eq!(c.key.as_deref(), Some("DEV-2"));
        assert_eq!(normalize_repos(&c.repos, &["api-pagamentos".into(), "web-app".into()]), vec!["api-pagamentos"]);
        let err = json!({"type": "result", "is_error": true, "result": "Credit balance is too low"}).to_string();
        assert!(parse_claude_output(&err).unwrap_err().contains("Credit balance"));
        // sem o envelope (texto cru)
        assert_eq!(parse_claude_output("{\"titulo\": \"X\"}").unwrap().title, "X");
    }

    #[test]
    fn prompt_e_argumentos() {
        let p = build_prompt("DEV-123", &["api-pagamentos".into(), "web-app".into()], true);
        assert!(p.contains("Card: DEV-123"));
        assert!(p.contains("- api-pagamentos\n- web-app"));
        assert!(p.contains("\"titulo\""));
        assert!(p.contains("SOMENTE LEITURA"));
        assert!(p.contains("threads do Slack"));
        let sem = build_prompt("x", &[], false);
        assert!(sem.contains("(nenhum configurado)") && sem.contains("Não use o Slack"));
        let a = claude_args(Some("haiku"));
        assert_eq!(&a[..3], ["-p", "--output-format", "json"]);
        assert!(a.windows(2).any(|w| w == ["--model", "haiku"]));
        let tools = &a[a.iter().position(|x| x == "--allowedTools").unwrap() + 1..];
        assert!(tools.iter().all(|t| !t.contains("create") && !t.contains("update") && !t.contains("send")));
        assert!(tools.contains(&"mcp__claude_ai_Notion__notion-fetch".to_string()));
    }

    #[test]
    fn repos_e_links_sem_ia() {
        let known = vec!["api-pagamentos".to_string(), "web".to_string(), "web-app".to_string()];
        assert_eq!(guess_repos("Mexer no API-PAGAMENTOS e no web-app", &known), vec!["api-pagamentos", "web-app"]);
        assert_eq!(slack_links("ver (https://a.slack.com/archives/C1/p9). e https://b.com"), vec!["https://a.slack.com/archives/C1/p9"]);
    }

    #[test]
    fn plano_com_worktree_e_shell() {
        let repos = vec![("api-pagamentos".to_string(), PathBuf::from("/r/api-pagamentos")), ("web-app".to_string(), PathBuf::from("/r/web-app"))];
        let opts = CreateOptions { worktree: true, shell: true, notes: true, plan_mode: true };
        let p = plan_creation(&card(), &repos, Path::new("/home/x"), &opts);
        assert_eq!(p.block_name, "DEV-1256 · Corrigir cálculo de juros no ch…");
        assert_eq!(p.branch, "DEV-1256-corrigir-calculo-de-juros-no-checkout");
        assert_eq!(p.sessions.len(), 3);
        let (a, b, sh) = (&p.sessions[0], &p.sessions[1], &p.sessions[2]);
        assert_eq!((a.kind, a.repo.as_deref(), a.branch.as_deref()), (PlannedKind::Claude, Some("api-pagamentos"), Some(p.branch.as_str())));
        assert_eq!(b.repo.as_deref(), Some("web-app"));
        assert_eq!(a.claude_args, vec!["--permission-mode", "plan"]);
        let prompt = a.prompt.as_deref().unwrap();
        assert!(prompt.contains("DEV-1256") && prompt.contains("@.aic/context.md") && prompt.contains("plano"));
        assert!(prompt.contains("web-app"), "cita os outros repos");
        assert_eq!((sh.kind, sh.repo.as_deref(), sh.branch.as_deref(), sh.prompt.as_deref()), (PlannedKind::Shell, Some("api-pagamentos"), None, None));
        assert!(p.notes.as_deref().unwrap().contains("- [ ] arredondar no fim"));
        assert!(p.context_md.contains("- Card: DEV-1256"));
    }

    #[test]
    fn plano_sem_repo_nem_worktree() {
        let mut c = card();
        c.key = None;
        let opts = CreateOptions { worktree: false, shell: false, notes: false, plan_mode: false };
        let p = plan_creation(&c, &[], Path::new("/home/x"), &opts);
        assert_eq!(p.sessions.len(), 1);
        let s = &p.sessions[0];
        assert_eq!((s.kind, s.path.as_path(), s.branch.as_deref()), (PlannedKind::Claude, Path::new("/home/x"), None));
        assert!(s.claude_args.is_empty());
        assert!(p.notes.is_none());
        assert_eq!(p.block_name, "Corrigir cálculo de juros no ch…");
    }

    #[test]
    fn resumo_do_painel() {
        let s = card().summary_text();
        assert!(s.starts_with("Em andamento · Fulano"));
        assert!(s.contains("• teste de regressão"));
    }
}

/// Busca real via `claude -p` (rede + conta do usuário). Só leitura.
///   AIC_CARD=DEV-123 cargo test -p aic-core busca_real -- --ignored --nocapture
#[cfg(test)]
#[test]
#[ignore]
fn busca_real() {
    let input = std::env::var("AIC_CARD").expect("defina AIC_CARD com o link ou a chave");
    let repos: Vec<String> = std::env::var("AIC_REPOS").unwrap_or_default().split(',').filter(|s| !s.is_empty()).map(String::from).collect();
    let t = Instant::now();
    let r = fetch(Connector::Claude, &input, &repos, &NotionConfig::default(), true, &AtomicBool::new(false));
    println!("{:.1}s\n{r:#?}", t.elapsed().as_secs_f32());
    assert!(r.is_ok());
}
