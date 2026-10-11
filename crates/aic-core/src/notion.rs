//! Cliente síncrono da API do Notion + mapeamento página → `Task` + upsert.
//!
//! - API: `Notion-Version: 2026-03-11` (a mais recente na doc oficial). Desde a 2025-09-03
//!   database e data source são coisas diferentes: query/schema/criação usam o
//!   **data source id**. Aqui aceitamos os dois — se o id não for de um data source,
//!   resolvemos via `GET /v1/databases/{id}` → `data_sources[0].id`.
//! - Tudo é bloqueante (reqwest blocking + rustls): chame de uma thread de fundo.
//! - O token nunca entra em mensagem de erro nem em log.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::time::Duration;

use anyhow::{Context, Result, anyhow, bail};
use chrono::Utc;
use serde_json::{Value, json};

use crate::config::{NotionConfig, NotionStatusMap};
use crate::model::{Priority, Task, TaskSource, TaskStatus};
use crate::paths;

pub const API_BASE: &str = "https://api.notion.com/v1";
pub const NOTION_VERSION: &str = "2026-03-11";

// ───────────────────────────── token ─────────────────────────────

/// `~/.config/aic/secrets.toml`
pub fn secrets_path() -> PathBuf {
    paths::config_dir().join("secrets.toml")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenSource {
    Env,
    File,
}

/// Token do Notion: env `NOTION_TOKEN` tem prioridade, depois `secrets.toml`.
pub fn load_token() -> Option<(String, TokenSource)> {
    if let Some(t) = std::env::var("NOTION_TOKEN").ok().map(|t| t.trim().to_string()).filter(|t| !t.is_empty()) {
        return Some((t, TokenSource::Env));
    }
    let text = std::fs::read_to_string(secrets_path()).ok()?;
    let v: toml::Table = toml::from_str(&text).ok()?;
    v.get("notion_token")
        .and_then(|t| t.as_str())
        .map(|t| t.trim().to_string())
        .filter(|t| !t.is_empty())
        .map(|t| (t, TokenSource::File))
}

/// Grava o token em `secrets.toml` com permissão 600 (preserva outras chaves). Vazio remove.
pub fn save_token(token: &str) -> Result<()> {
    let path = secrets_path();
    if let Some(d) = path.parent() {
        std::fs::create_dir_all(d)?;
    }
    let mut table: toml::Table = std::fs::read_to_string(&path).ok().and_then(|t| toml::from_str(&t).ok()).unwrap_or_default();
    let token = token.trim();
    if token.is_empty() {
        table.remove("notion_token");
    } else {
        table.insert("notion_token".into(), toml::Value::String(token.to_string()));
    }
    let text = toml::to_string(&table)?;
    let tmp = path.with_extension("toml.tmp");
    write_private(&tmp, &text)?;
    std::fs::rename(&tmp, &path)?;
    set_private(&path)?;
    Ok(())
}

#[cfg(unix)]
fn write_private(path: &std::path::Path, text: &str) -> Result<()> {
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;
    let _ = std::fs::remove_file(path);
    let mut f = std::fs::OpenOptions::new().write(true).create_new(true).mode(0o600).open(path)?;
    f.write_all(text.as_bytes())?;
    Ok(())
}

#[cfg(not(unix))]
fn write_private(path: &std::path::Path, text: &str) -> Result<()> {
    std::fs::write(path, text)?;
    Ok(())
}

#[cfg(unix)]
fn set_private(path: &std::path::Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
    Ok(())
}

#[cfg(not(unix))]
fn set_private(_: &std::path::Path) -> Result<()> {
    Ok(())
}

// ───────────────────────────── ids ─────────────────────────────

/// Id do Notion normalizado: 32 hex minúsculos, sem hífens.
pub fn norm_id(s: &str) -> String {
    s.chars().filter(|c| *c != '-').collect::<String>().to_ascii_lowercase()
}

/// Extrai o id de uma página a partir de URL (`…/Titulo-<32hex>?…`) ou de um id cru.
pub fn page_id_from(s: &str) -> Option<String> {
    let base = s.split(['?', '#']).next().unwrap_or(s);
    let last = base.rsplit('/').next().unwrap_or(base);
    let compact = norm_id(last);
    let tail: String = compact.chars().rev().take(32).collect::<Vec<_>>().into_iter().rev().collect();
    (tail.len() == 32 && tail.chars().all(|c| c.is_ascii_hexdigit())).then_some(tail)
}

fn looks_like_id(s: &str) -> bool {
    let n = norm_id(s);
    n.len() == 32 && n.chars().all(|c| c.is_ascii_hexdigit())
}

fn opt(s: &Option<String>) -> Option<&str> {
    s.as_deref().map(str::trim).filter(|s| !s.is_empty())
}

// ───────────────────────────── mapeamento ─────────────────────────────

/// Um card do Notion já traduzido pro vocabulário do AIC.
#[derive(Debug, Clone, PartialEq)]
pub struct NotionCard {
    /// Page id normalizado (32 hex).
    pub page_id: String,
    pub url: Option<String>,
    pub title: String,
    pub key: Option<String>,
    pub status: TaskStatus,
    pub status_name: Option<String>,
    pub priority: Option<Priority>,
    /// Ids (normalizados) dos responsáveis.
    pub assignees: Vec<String>,
    /// Ids (normalizados) das sprints relacionadas.
    pub sprints: Vec<String>,
    /// Tipo do card (ex: "❇️Story").
    pub kind: Option<String>,
}

fn plain_text(arr: Option<&Value>) -> String {
    arr.and_then(Value::as_array)
        .map(|a| a.iter().filter_map(|t| t.get("plain_text").and_then(Value::as_str)).collect::<String>())
        .unwrap_or_default()
}

/// Nome de um status/select ou `None`.
fn option_name(prop: &Value) -> Option<String> {
    let ty = prop.get("type")?.as_str()?;
    prop.get(ty)?.get("name")?.as_str().map(str::to_string)
}

/// Status do Notion → coluna do AIC. Primeiro o mapa configurado, depois heurística.
pub fn map_status(name: &str, map: &NotionStatusMap) -> TaskStatus {
    let eq = |list: &Vec<String>| list.iter().any(|s| s.trim().eq_ignore_ascii_case(name.trim()));
    for (list, st) in [
        (&map.archived, TaskStatus::Archived),
        (&map.done, TaskStatus::Done),
        (&map.review, TaskStatus::Review),
        (&map.doing, TaskStatus::Doing),
        (&map.todo, TaskStatus::Todo),
    ] {
        if eq(list) {
            return st;
        }
    }
    let n = name.to_lowercase();
    let has = |ks: &[&str]| ks.iter().any(|k| n.contains(k));
    if has(&["arquiv", "archiv", "cancel"]) {
        TaskStatus::Archived
    } else if has(&["conclu", "done", "feito", "complete", "finaliz"]) {
        TaskStatus::Done
    } else if has(&["review", "revis", "q.a", "qa", "deploy", "test", "check"]) {
        TaskStatus::Review
    } else if has(&["andamento", "progress", "doing", "fazendo", "corre"]) {
        TaskStatus::Doing
    } else {
        TaskStatus::Todo
    }
}

/// Coluna do AIC → nome de status a gravar no Notion (o primeiro da lista).
pub fn status_name_for(st: TaskStatus, map: &NotionStatusMap) -> Option<&str> {
    let list = match st {
        TaskStatus::Todo => &map.todo,
        TaskStatus::Doing => &map.doing,
        TaskStatus::Review => &map.review,
        TaskStatus::Done => &map.done,
        TaskStatus::Archived => &map.archived,
    };
    list.first().map(String::as_str).filter(|s| !s.trim().is_empty())
}

pub fn map_priority(name: &str) -> Priority {
    let n = name.to_lowercase();
    if n.contains("urg") || n.contains("crít") || n.contains("crit") {
        Priority::Urgent
    } else if n.contains("alt") || n.contains("high") {
        Priority::High
    } else if n.contains("baix") || n.contains("low") {
        Priority::Low
    } else {
        Priority::Normal
    }
}

/// Página (objeto `page` da API) → card. `None` se não for página.
pub fn page_to_card(page: &Value, cfg: &NotionConfig) -> Option<NotionCard> {
    if page.get("object").and_then(Value::as_str) != Some("page") {
        return None;
    }
    let page_id = norm_id(page.get("id")?.as_str()?);
    let props = page.get("properties").and_then(Value::as_object)?;
    let get = |name: &Option<String>| opt(name).and_then(|n| props.get(n));

    let title_prop = get(&cfg.title_property)
        .filter(|p| p.get("type").and_then(Value::as_str) == Some("title"))
        .or_else(|| props.values().find(|p| p.get("type").and_then(Value::as_str) == Some("title")));
    let title = title_prop.map(|p| plain_text(p.get("title"))).unwrap_or_default().trim().to_string();

    let key = get(&cfg.key_property)
        .or_else(|| props.values().find(|p| p.get("type").and_then(Value::as_str) == Some("unique_id")))
        .and_then(|p| p.get("unique_id"))
        .and_then(|u| {
            let n = u.get("number")?.as_i64()?;
            let prefix = u.get("prefix").and_then(Value::as_str).map(str::to_string).or_else(|| opt(&cfg.key_prefix).map(str::to_string));
            Some(match prefix {
                Some(p) if !p.is_empty() => format!("{p}-{n}"),
                _ => n.to_string(),
            })
        });

    let status_name = get(&cfg.status_property).and_then(option_name);
    let status = status_name.as_deref().map(|n| map_status(n, &cfg.status_map)).unwrap_or_default();
    let priority = get(&cfg.priority_property).and_then(option_name).map(|n| map_priority(&n));
    let kind = get(&cfg.type_property).and_then(option_name);

    let ids = |p: Option<&Value>, field: &str| -> Vec<String> {
        p.and_then(|p| p.get(field)).and_then(Value::as_array).map(|a| a.iter().filter_map(|x| x.get("id")?.as_str().map(norm_id)).collect()).unwrap_or_default()
    };
    let assignees = ids(get(&cfg.assignee_property), "people");
    let sprints = ids(get(&cfg.sprint_property), "relation");

    Some(NotionCard {
        page_id,
        url: page.get("url").and_then(Value::as_str).map(str::to_string),
        title: if title.is_empty() { "(sem título)".into() } else { title },
        key,
        status,
        status_name,
        priority,
        assignees,
        sprints,
        kind,
    })
}

/// Tag curta a partir do tipo ("❇️Story" → "story", "🐞BUG" → "bug").
pub fn kind_tag(kind: &str) -> Option<String> {
    let t: String = kind.chars().filter(|c| c.is_alphanumeric() || *c == '-' || *c == ' ').collect::<String>().trim().to_lowercase().replace(' ', "-");
    (!t.is_empty()).then_some(t)
}

// ───────────────────────────── upsert ─────────────────────────────

#[derive(Debug, Default)]
pub struct UpsertPlan {
    /// Tarefas a gravar (novas ou alteradas).
    pub save: Vec<Task>,
    pub created: usize,
    pub updated: usize,
    pub archived: usize,
}

/// Junta os cards do Notion com as tarefas locais, sem duplicar:
/// - casa por `notion_page` (ou pelo id contido em `notion_url` de uma tarefa local);
/// - atualiza só o que vem do Notion (título, status, prioridade, chave, url) e preserva
///   vínculos locais (sessão, workspace, repo, notas, subtarefas, rank, tags);
/// - card que sumiu do resultado é arquivado (vínculos ficam).
pub fn plan_upsert(existing: &[Task], cards: &[NotionCard]) -> UpsertPlan {
    let mut plan = UpsertPlan::default();
    let mut by_page: HashMap<String, usize> = HashMap::new();
    for (i, t) in existing.iter().enumerate() {
        if let Some(p) = t.notion_page.as_deref().and_then(page_id_from) {
            by_page.entry(p).or_insert(i);
        }
    }
    for (i, t) in existing.iter().enumerate() {
        if t.notion_page.is_none()
            && let Some(p) = t.notion_url.as_deref().and_then(page_id_from)
        {
            by_page.entry(p).or_insert(i);
        }
    }

    let mut seen: HashSet<String> = HashSet::new();
    for c in cards {
        if !seen.insert(c.page_id.clone()) {
            continue;
        }
        match by_page.get(&c.page_id) {
            Some(&i) => {
                let before = &existing[i];
                let mut t = before.clone();
                apply_card(&mut t, c);
                if t != *before {
                    t.updated_at = Utc::now();
                    plan.updated += 1;
                    plan.save.push(t);
                }
            }
            None => {
                let mut t = Task::new(c.title.clone());
                apply_card(&mut t, c);
                plan.created += 1;
                plan.save.push(t);
            }
        }
    }

    for t in existing {
        let Some(p) = t.notion_page.as_deref().and_then(page_id_from) else { continue };
        if t.source == TaskSource::Notion && !seen.contains(&p) && t.status != TaskStatus::Archived {
            let mut t = t.clone();
            t.status = TaskStatus::Archived;
            t.touch();
            plan.archived += 1;
            plan.save.push(t);
        }
    }
    plan
}

fn apply_card(t: &mut Task, c: &NotionCard) {
    t.source = TaskSource::Notion;
    t.notion_page = Some(c.page_id.clone());
    if c.url.is_some() {
        t.notion_url = c.url.clone();
    }
    t.title = c.title.clone();
    t.status = c.status;
    if let Some(p) = c.priority {
        t.priority = p;
    }
    if c.key.is_some() {
        t.card_key = c.key.clone();
    }
    if let Some(tag) = c.kind.as_deref().and_then(kind_tag)
        && !t.tags.contains(&tag)
    {
        t.tags.push(tag);
    }
}

// ───────────────────────────── filtro ─────────────────────────────

/// Filtro da query: `me` responsável e/ou relação com a sprint atual (AND).
pub fn build_filter(cfg: &NotionConfig, me_id: Option<&str>, sprint_id: Option<&str>) -> Option<Value> {
    let mut parts = vec![];
    if let (Some(me), Some(prop)) = (me_id, opt(&cfg.assignee_property)) {
        parts.push(json!({"property": prop, "people": {"contains": me}}));
    }
    if let (Some(sp), Some(prop)) = (sprint_id, opt(&cfg.sprint_property)) {
        parts.push(json!({"property": prop, "relation": {"contains": sp}}));
    }
    match parts.len() {
        0 => None,
        1 => parts.pop(),
        _ => Some(json!({"and": parts})),
    }
}

// ───────────────────────────── detecção de schema ─────────────────────────────

/// Mapeamento sugerido a partir do schema do data source.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Detected {
    pub data_source_id: String,
    pub name: String,
    pub title: Option<String>,
    pub status: Option<String>,
    pub key: Option<String>,
    pub key_prefix: Option<String>,
    pub assignee: Option<String>,
    pub sprint: Option<String>,
    pub priority: Option<String>,
    pub kind: Option<String>,
    pub status_map: Option<NotionStatusMap>,
}

impl Detected {
    /// Aplica a sugestão na config (só o que foi detectado).
    pub fn apply(&self, cfg: &mut NotionConfig) {
        cfg.tasks_database = Some(self.data_source_id.clone());
        let set = |dst: &mut Option<String>, v: &Option<String>| {
            if v.is_some() {
                *dst = v.clone();
            }
        };
        set(&mut cfg.title_property, &self.title);
        set(&mut cfg.status_property, &self.status);
        set(&mut cfg.key_property, &self.key);
        set(&mut cfg.key_prefix, &self.key_prefix);
        set(&mut cfg.assignee_property, &self.assignee);
        set(&mut cfg.sprint_property, &self.sprint);
        set(&mut cfg.priority_property, &self.priority);
        set(&mut cfg.type_property, &self.kind);
        if let Some(m) = &self.status_map {
            cfg.status_map = m.clone();
        }
    }
}

/// Lê o schema (`GET /v1/data_sources/{id}`) e sugere propriedades.
pub fn detect_schema(ds: &Value) -> Detected {
    let mut d = Detected {
        data_source_id: ds.get("id").and_then(Value::as_str).unwrap_or_default().to_string(),
        name: plain_text(ds.get("title")),
        ..Default::default()
    };
    let Some(props) = ds.get("properties").and_then(Value::as_object) else { return d };
    let mut entries: Vec<(&String, &Value)> = props.iter().collect();
    entries.sort_by_key(|(k, _)| k.to_lowercase());
    let ty = |p: &Value| p.get("type").and_then(Value::as_str).unwrap_or_default().to_string();
    // Por tipo, preferindo nomes conhecidos.
    let pick = |types: &[&str], names: &[&str]| -> Option<String> {
        let cands: Vec<&String> = entries.iter().filter(|(_, p)| types.contains(&ty(p).as_str())).map(|(k, _)| *k).collect();
        if names.is_empty() {
            return cands.first().map(|k| k.to_string());
        }
        // Nome exato > começa com > contém (evita "Q.A responsável" no lugar de "Responsável").
        let score = |k: &str| -> Option<u8> {
            let k = k.to_lowercase();
            names.iter().filter_map(|n| if k == *n { Some(0) } else if k.starts_with(n) { Some(1) } else if k.contains(n) { Some(2) } else { None }).min()
        };
        cands.iter().filter_map(|k| score(k).map(|s| (s, *k))).min_by_key(|(s, _)| *s).map(|(_, k)| k.to_string())
    };
    d.title = pick(&["title"], &[]);
    d.status = pick(&["status"], &["status"]).or_else(|| pick(&["status"], &[])).or_else(|| pick(&["select"], &["status", "estado"]));
    d.key = pick(&["unique_id"], &[]);
    d.key_prefix = d.key.as_ref().and_then(|k| props.get(k)).and_then(|p| p.get("unique_id")?.get("prefix")?.as_str().map(str::to_string));
    d.assignee = pick(&["people"], &["respons", "assignee", "assign", "dono", "owner"]).or_else(|| pick(&["people"], &[]));
    d.sprint = pick(&["relation"], &["sprint"]);
    d.priority = pick(&["select", "status"], &["priori"]);
    d.kind = pick(&["select"], &["tipo", "type"]);

    // Mapa de status a partir dos grupos do Notion (To-do / In progress / Complete).
    if let Some(sp) = d.status.as_ref().and_then(|k| props.get(k)).and_then(|p| p.get("status")) {
        let options: HashMap<&str, &str> = sp
            .get("options")
            .and_then(Value::as_array)
            .map(|a| a.iter().filter_map(|o| Some((o.get("id")?.as_str()?, o.get("name")?.as_str()?))).collect())
            .unwrap_or_default();
        let mut m = NotionStatusMap { todo: vec![], doing: vec![], review: vec![], done: vec![], archived: vec![] };
        for g in sp.get("groups").and_then(Value::as_array).into_iter().flatten() {
            let gname = g.get("name").and_then(Value::as_str).unwrap_or_default().to_lowercase();
            for oid in g.get("option_ids").and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_str) {
                let Some(name) = options.get(oid).map(|s| s.to_string()) else { continue };
                let guess = map_status(&name, &NotionStatusMap { todo: vec![], doing: vec![], review: vec![], done: vec![], archived: vec![] });
                let st = if gname.contains("complete") || gname.contains("conclu") {
                    if guess == TaskStatus::Archived { TaskStatus::Archived } else { TaskStatus::Done }
                } else if gname.contains("progress") || gname.contains("andamento") {
                    if guess == TaskStatus::Review { TaskStatus::Review } else { TaskStatus::Doing }
                } else {
                    TaskStatus::Todo
                };
                match st {
                    TaskStatus::Todo => m.todo.push(name),
                    TaskStatus::Doing => m.doing.push(name),
                    TaskStatus::Review => m.review.push(name),
                    TaskStatus::Done => m.done.push(name),
                    TaskStatus::Archived => m.archived.push(name),
                }
            }
        }
        // "Sprint backlog" antes de "Backlog": o card promovido entra na sprint.
        m.todo.sort_by_key(|n| !n.to_lowercase().contains("sprint"));
        if !(m.todo.is_empty() && m.doing.is_empty() && m.done.is_empty()) {
            d.status_map = Some(m);
        }
    }
    d
}

// ───────────────────────────── cliente HTTP ─────────────────────────────

pub struct Client {
    http: reqwest::blocking::Client,
    token: String,
    base: String,
}

/// Resultado de um sync.
#[derive(Debug, Clone, Default)]
pub struct SyncOutcome {
    pub cards: Vec<NotionCard>,
    pub me_id: Option<String>,
    pub sprint: Option<(String, String)>,
    /// Avisos não fatais (ex: sprint atual não encontrada).
    pub notes: Vec<String>,
}

impl Client {
    pub fn new(token: impl Into<String>) -> Result<Self> {
        let http = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(30))
            .user_agent(concat!("aic/", env!("CARGO_PKG_VERSION")))
            .build()?;
        Ok(Self { http, token: token.into(), base: API_BASE.into() })
    }

    /// Cliente a partir do token salvo/env.
    pub fn from_saved() -> Result<Self> {
        let (t, _) = load_token().ok_or_else(|| anyhow!("token do Notion não configurado (Config → Integrações ou env NOTION_TOKEN)"))?;
        Self::new(t)
    }

    fn call(&self, method: reqwest::Method, path: &str, body: Option<&Value>) -> Result<Value> {
        let url = format!("{}{}", self.base, path);
        let mut req = self.http.request(method, &url).bearer_auth(&self.token).header("Notion-Version", NOTION_VERSION);
        if let Some(b) = body {
            req = req.json(b);
        }
        let resp = req.send().map_err(|e| anyhow!("falha de rede falando com o Notion: {}", e.without_url()))?;
        let status = resp.status();
        let v: Value = resp.json().unwrap_or(Value::Null);
        if !status.is_success() {
            let code = v.get("code").and_then(Value::as_str).unwrap_or("erro");
            let msg = v.get("message").and_then(Value::as_str).unwrap_or("");
            bail!(NotionError { status: status.as_u16(), code: code.to_string(), message: msg.to_string() });
        }
        Ok(v)
    }

    fn get(&self, path: &str) -> Result<Value> {
        self.call(reqwest::Method::GET, path, None)
    }

    /// Bot da integração (`GET /v1/users/me`): serve de "testar conexão".
    pub fn me_bot(&self) -> Result<String> {
        let v = self.get("/users/me")?;
        Ok(v.get("name").and_then(Value::as_str).unwrap_or("integração").to_string())
    }

    /// Schema do data source. Aceita id de database (resolve pro primeiro data source).
    pub fn data_source(&self, id: &str) -> Result<Value> {
        let id = norm_id(id);
        match self.get(&format!("/data_sources/{id}")) {
            Ok(v) => Ok(v),
            Err(e) if is_status(&e, &[400, 404]) => {
                let db = self.get(&format!("/databases/{id}")).map_err(|_| e)?;
                let ds = db
                    .get("data_sources")
                    .and_then(Value::as_array)
                    .and_then(|a| a.first())
                    .and_then(|d| d.get("id"))
                    .and_then(Value::as_str)
                    .ok_or_else(|| anyhow!("database sem data source acessível"))?
                    .to_string();
                self.get(&format!("/data_sources/{}", norm_id(&ds)))
            }
            Err(e) => Err(e),
        }
    }

    /// Todas as páginas da query (paginação por `start_cursor`).
    pub fn query_all(&self, data_source_id: &str, filter: Option<Value>) -> Result<Vec<Value>> {
        let mut out = vec![];
        let mut cursor: Option<String> = None;
        for _ in 0..100 {
            let mut body = json!({"page_size": 100, "result_type": "page"});
            if let Some(f) = &filter {
                body["filter"] = f.clone();
            }
            if let Some(c) = &cursor {
                body["start_cursor"] = json!(c);
            }
            let v = self.call(reqwest::Method::POST, &format!("/data_sources/{}/query", norm_id(data_source_id)), Some(&body))?;
            out.extend(v.get("results").and_then(Value::as_array).cloned().unwrap_or_default());
            cursor = v.get("next_cursor").and_then(Value::as_str).map(str::to_string);
            if !v.get("has_more").and_then(Value::as_bool).unwrap_or(false) || cursor.is_none() {
                break;
            }
        }
        Ok(out)
    }

    /// `me` (e-mail ou id) → id do usuário. E-mail exige a capability "ler info de usuário com e-mail".
    pub fn resolve_user(&self, me: &str) -> Result<String> {
        let me = me.trim();
        if looks_like_id(me) {
            return Ok(norm_id(me));
        }
        let mut cursor: Option<String> = None;
        for _ in 0..50 {
            let path = match &cursor {
                Some(c) => format!("/users?page_size=100&start_cursor={c}"),
                None => "/users?page_size=100".into(),
            };
            let v = self.get(&path)?;
            for u in v.get("results").and_then(Value::as_array).into_iter().flatten() {
                let email = u.get("person").and_then(|p| p.get("email")).and_then(Value::as_str);
                if email.is_some_and(|e| e.eq_ignore_ascii_case(me))
                    && let Some(id) = u.get("id").and_then(Value::as_str)
                {
                    return Ok(norm_id(id));
                }
            }
            cursor = v.get("next_cursor").and_then(Value::as_str).map(str::to_string);
            if !v.get("has_more").and_then(Value::as_bool).unwrap_or(false) || cursor.is_none() {
                break;
            }
        }
        bail!("usuário '{me}' não encontrado no Notion (use o id, ou dê à integração acesso a e-mails)")
    }

    /// Sprint atual: página da database de sprints com `<status> = <valor atual>`.
    pub fn current_sprint(&self, cfg: &NotionConfig) -> Result<Option<(String, String)>> {
        let (Some(db), Some(prop), Some(val)) = (opt(&cfg.sprints_database), opt(&cfg.sprint_status_property), opt(&cfg.sprint_current_value)) else {
            return Ok(None);
        };
        let ds = self.data_source(db)?;
        let ds_id = ds.get("id").and_then(Value::as_str).unwrap_or(db).to_string();
        let kind = ds.get("properties").and_then(|p| p.get(prop)).and_then(|p| p.get("type")).and_then(Value::as_str).unwrap_or("status").to_string();
        let filter = json!({"property": prop, kind.clone(): {"equals": val}});
        let pages = self.query_all(&ds_id, Some(filter))?;
        Ok(pages.first().map(|p| {
            let id = norm_id(p.get("id").and_then(Value::as_str).unwrap_or_default());
            let name = p
                .get("properties")
                .and_then(Value::as_object)
                .and_then(|o| o.values().find(|v| v.get("type").and_then(Value::as_str) == Some("title")))
                .map(|t| plain_text(t.get("title")))
                .unwrap_or_default();
            (id, name)
        }))
    }

    /// Sync completo: resolve `me` e a sprint, monta o filtro e traz os cards.
    pub fn sync(&self, cfg: &NotionConfig) -> Result<SyncOutcome> {
        let db = opt(&cfg.tasks_database).ok_or_else(|| anyhow!("database de tasks não configurada"))?;
        let ds = self.data_source(db).context("lendo a database de tasks")?;
        let ds_id = ds.get("id").and_then(Value::as_str).unwrap_or(db).to_string();
        let mut out = SyncOutcome::default();

        if cfg.only_mine {
            let me = opt(&cfg.me).ok_or_else(|| anyhow!("filtro 'só meus cards' ligado, mas 'me' está vazio"))?;
            out.me_id = Some(self.resolve_user(me)?);
        }
        if cfg.only_current_sprint {
            match self.current_sprint(cfg) {
                Ok(Some(s)) => out.sprint = Some(s),
                Ok(None) => out.notes.push("sprint atual não encontrada; filtrando só por responsável".into()),
                Err(e) => out.notes.push(format!("não deu pra ler a sprint atual ({e}); filtrando só por responsável")),
            }
        }
        let filter = build_filter(cfg, out.me_id.as_deref(), out.sprint.as_ref().map(|s| s.0.as_str()));
        if filter.is_none() && (cfg.only_mine || cfg.only_current_sprint) {
            bail!("nenhum filtro aplicável (configure 'me' ou a database de sprints) — sync abortado pra não trazer o backlog inteiro");
        }
        let pages = self.query_all(&ds_id, filter)?;
        out.cards = pages.iter().filter_map(|p| page_to_card(p, cfg)).collect();
        Ok(out)
    }

    /// Atualiza o status de uma página. Tenta como `status`; se a propriedade for `select`, refaz.
    pub fn update_status(&self, page_id: &str, cfg: &NotionConfig, st: TaskStatus) -> Result<String> {
        let prop = opt(&cfg.status_property).ok_or_else(|| anyhow!("propriedade de status não configurada"))?;
        let name = status_name_for(st, &cfg.status_map).ok_or_else(|| anyhow!("sem status do Notion mapeado para '{}'", st.label()))?.to_string();
        let path = format!("/pages/{}", norm_id(page_id));
        let body = json!({"properties": {prop: {"status": {"name": name}}}});
        match self.call(reqwest::Method::PATCH, &path, Some(&body)) {
            Ok(_) => Ok(name),
            Err(e) if is_status(&e, &[400]) => {
                let body = json!({"properties": {prop: {"select": {"name": name}}}});
                self.call(reqwest::Method::PATCH, &path, Some(&body)).map_err(|_| e)?;
                Ok(name)
            }
            Err(e) => Err(e),
        }
    }

    /// Cria um card a partir de uma tarefa local. Já entra com `me` e a sprint atual quando os
    /// filtros estão ligados — senão o próximo sync arquivaria o card recém-criado.
    pub fn create_card(&self, cfg: &NotionConfig, task: &Task) -> Result<NotionCard> {
        let db = opt(&cfg.tasks_database).ok_or_else(|| anyhow!("database de tasks não configurada"))?;
        let ds = self.data_source(db)?;
        let ds_id = ds.get("id").and_then(Value::as_str).unwrap_or(db).to_string();
        let schema = ds.get("properties").and_then(Value::as_object).cloned().unwrap_or_default();
        let title_prop = opt(&cfg.title_property)
            .filter(|n| schema.get(*n).and_then(|p| p.get("type")).and_then(Value::as_str) == Some("title"))
            .map(str::to_string)
            .or_else(|| schema.iter().find(|(_, p)| p.get("type").and_then(Value::as_str) == Some("title")).map(|(k, _)| k.clone()))
            .ok_or_else(|| anyhow!("database sem propriedade de título"))?;

        let mut props = serde_json::Map::new();
        props.insert(title_prop, json!({"title": [{"type": "text", "text": {"content": task.title}}]}));
        if let (Some(prop), Some(name)) = (opt(&cfg.status_property), status_name_for(task.status, &cfg.status_map)) {
            let kind = schema.get(prop).and_then(|p| p.get("type")).and_then(Value::as_str).unwrap_or("status");
            props.insert(prop.into(), json!({kind: {"name": name}}));
        }
        if let (Some(prop), Some(me)) = (opt(&cfg.assignee_property), opt(&cfg.me)) {
            props.insert(prop.into(), json!({"people": [{"id": self.resolve_user(me)?}]}));
        }
        if cfg.only_current_sprint
            && let Some(prop) = opt(&cfg.sprint_property)
            && let Ok(Some((sid, _))) = self.current_sprint(cfg)
        {
            props.insert(prop.into(), json!({"relation": [{"id": sid}]}));
        }
        let mut body = json!({"parent": {"type": "data_source_id", "data_source_id": ds_id}, "properties": props});
        if !task.notes.trim().is_empty() {
            body["children"] = json!([{"object": "block", "type": "paragraph", "paragraph": {"rich_text": [{"type": "text", "text": {"content": truncate(&task.notes, 1900)}}]}}]);
        }
        let page = self.call(reqwest::Method::POST, "/pages", Some(&body))?;
        page_to_card(&page, cfg).ok_or_else(|| anyhow!("resposta inesperada do Notion ao criar a página"))
    }
}

fn truncate(s: &str, max: usize) -> String {
    s.chars().take(max).collect()
}

/// Erro HTTP do Notion (sem token, sem URL).
#[derive(Debug)]
pub struct NotionError {
    pub status: u16,
    pub code: String,
    pub message: String,
}

impl std::fmt::Display for NotionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let hint = match self.status {
            401 => " — token inválido",
            403 => " — a integração não tem essa permissão",
            404 => " — compartilhe a database com a integração (••• → Conexões)",
            429 => " — limite de requisições, tente de novo em instantes",
            _ => "",
        };
        write!(f, "Notion {} {}: {}{}", self.status, self.code, self.message, hint)
    }
}

impl std::error::Error for NotionError {}

fn is_status(e: &anyhow::Error, codes: &[u16]) -> bool {
    e.downcast_ref::<NotionError>().is_some_and(|n| codes.contains(&n.status))
}

// ───────────────────────────── testes ─────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    const PAGE_ID: &str = "1a2b3c4d-0000-4000-8000-00000000abcd";
    const ME: &str = "aaaaaaaa-1111-4111-8111-111111111111";
    const SPRINT: &str = "bbbbbbbb-2222-4222-8222-222222222222";

    fn fixture_query() -> Value {
        serde_json::from_str(include_str!("../tests/fixtures/notion_query.json")).unwrap()
    }

    /// Config com os nomes de propriedades e status das fixtures.
    fn fixture_cfg() -> NotionConfig {
        let mut cfg = NotionConfig::default();
        cfg.key_prefix = Some("DEV".into());
        cfg.title_property = Some("User story".into());
        cfg.status_map.todo.extend(["Sprint backlog".to_string()]);
        cfg.status_map.doing.extend(["Correções".to_string()]);
        cfg.status_map.review.extend(["Codereview".to_string(), "Waiting Q.A".to_string(), "Q.A".to_string()]);
        cfg
    }

    fn cards() -> Vec<NotionCard> {
        let cfg = fixture_cfg();
        fixture_query()["results"].as_array().unwrap().iter().filter_map(|p| page_to_card(p, &cfg)).collect()
    }

    #[test]
    fn mapeia_pagina_para_card() {
        let c = cards();
        assert_eq!(c.len(), 3, "o data_source na lista é ignorado");
        let a = &c[0];
        assert_eq!(a.page_id, norm_id(PAGE_ID));
        assert_eq!(a.title, "Story - Exportar relatório em CSV");
        assert_eq!(a.key.as_deref(), Some("DEV-142"), "prefixo do config quando o unique_id não tem");
        assert_eq!(a.status, TaskStatus::Doing);
        assert_eq!(a.status_name.as_deref(), Some("Em andamento"));
        assert_eq!(a.priority, Some(Priority::High));
        assert_eq!(a.assignees, vec![norm_id(ME)]);
        assert_eq!(a.sprints, vec![norm_id(SPRINT)]);
        assert_eq!(a.kind.as_deref(), Some("❇️Story"));
        assert!(a.url.as_deref().unwrap().starts_with("https://www.notion.so/"));

        // prefixo vindo do próprio unique_id; status de revisão; prioridade urgente
        assert_eq!(c[1].key.as_deref(), Some("BUG-7"));
        assert_eq!(c[1].status, TaskStatus::Review);
        assert_eq!(c[1].priority, Some(Priority::Urgent));
        // status desconhecido cai na heurística; título vazio vira placeholder
        assert_eq!(c[2].status, TaskStatus::Done);
        assert_eq!(c[2].title, "(sem título)");
        assert_eq!(c[2].priority, None);
    }

    #[test]
    fn status_ida_e_volta() {
        let m = NotionStatusMap::default();
        for st in [TaskStatus::Todo, TaskStatus::Doing, TaskStatus::Review, TaskStatus::Done, TaskStatus::Archived] {
            let name = status_name_for(st, &m).unwrap();
            assert_eq!(map_status(name, &m), st, "{name}");
        }
        assert_eq!(map_status("Backlog", &m), TaskStatus::Todo);
        assert_eq!(map_status("Q.A", &m), TaskStatus::Review);
        assert_eq!(map_status("In progress", &m), TaskStatus::Doing);
    }

    #[test]
    fn upsert_cria_atualiza_preserva_e_arquiva() {
        let cards = cards();
        // 1º sync: tudo novo
        let plan = plan_upsert(&[], &cards);
        assert_eq!((plan.created, plan.updated, plan.archived), (3, 0, 0));
        let mut tasks = plan.save;
        assert!(tasks.iter().all(|t| t.source == TaskSource::Notion && t.notion_page.is_some()));
        assert!(tasks[0].tags.contains(&"story".to_string()));

        // vínculos locais
        let sid = crate::model::new_id();
        tasks[0].session = Some(sid);
        tasks[0].notes = "nota local".into();
        tasks[0].subtasks.push(crate::model::Subtask { text: "sub".into(), done: false });

        // 2º sync idêntico: nada muda, nada duplica
        let plan = plan_upsert(&tasks, &cards);
        assert_eq!((plan.created, plan.updated, plan.archived), (0, 0, 0));

        // 3º sync: card 0 mudou de status, card 2 saiu do filtro
        let mut changed = cards.clone();
        changed[0].status = TaskStatus::Review;
        changed[0].title = "Novo título".into();
        changed.remove(2);
        let plan = plan_upsert(&tasks, &changed);
        assert_eq!((plan.created, plan.updated, plan.archived), (0, 1, 1));
        let t0 = plan.save.iter().find(|t| t.id == tasks[0].id).unwrap();
        assert_eq!(t0.status, TaskStatus::Review);
        assert_eq!(t0.title, "Novo título");
        assert_eq!(t0.session, Some(sid));
        assert_eq!(t0.notes, "nota local");
        assert_eq!(t0.subtasks.len(), 1);
        let t2 = plan.save.iter().find(|t| t.id == tasks[2].id).unwrap();
        assert_eq!(t2.status, TaskStatus::Archived);
        assert!(t2.notion_page.is_some(), "arquivar não apaga o vínculo");
    }

    #[test]
    fn upsert_vincula_tarefa_local_pela_url() {
        let cards = cards();
        let mut local = Task::new("rascunho local");
        local.notion_url = Some(format!("https://www.notion.so/ws/Story-{}?pvs=4", norm_id(PAGE_ID)));
        let plan = plan_upsert(std::slice::from_ref(&local), &cards[..1]);
        assert_eq!((plan.created, plan.updated), (0, 1));
        assert_eq!(plan.save[0].id, local.id);
        assert_eq!(plan.save[0].source, TaskSource::Notion);
        // Local sem vínculo nunca é arquivada
        let plan = plan_upsert(&[Task::new("só local")], &[]);
        assert_eq!(plan.archived, 0);
    }

    #[test]
    fn filtro() {
        let cfg = NotionConfig::default();
        assert_eq!(build_filter(&cfg, None, None), None);
        assert_eq!(build_filter(&cfg, Some("u"), None), Some(json!({"property": "Responsável", "people": {"contains": "u"}})));
        let f = build_filter(&cfg, Some("u"), Some("s")).unwrap();
        assert_eq!(f["and"].as_array().unwrap().len(), 2);
        assert_eq!(f["and"][1], json!({"property": "Sprint", "relation": {"contains": "s"}}));
    }

    #[test]
    fn detecta_schema() {
        let ds: Value = serde_json::from_str(include_str!("../tests/fixtures/notion_data_source.json")).unwrap();
        let d = detect_schema(&ds);
        assert_eq!(d.name, "Backlog - Time");
        assert_eq!(d.title.as_deref(), Some("User story"));
        assert_eq!(d.status.as_deref(), Some("Status"));
        assert_eq!(d.key.as_deref(), Some("ID"));
        assert_eq!(d.assignee.as_deref(), Some("Responsável"));
        assert_eq!(d.sprint.as_deref(), Some("Sprint"));
        assert_eq!(d.priority.as_deref(), Some("Prioridade"));
        assert_eq!(d.kind.as_deref(), Some("Tipo"));
        let m = d.status_map.clone().unwrap();
        assert_eq!(m.todo.first().map(String::as_str), Some("Sprint backlog"));
        assert!(m.review.contains(&"Codereview".to_string()));
        assert!(m.doing.contains(&"Em andamento".to_string()));
        assert_eq!(m.done, vec!["Concluído".to_string()]);
        assert_eq!(m.archived, vec!["Arquivado".to_string()]);
        let mut cfg = NotionConfig { tasks_database: None, ..Default::default() };
        d.apply(&mut cfg);
        assert_eq!(cfg.tasks_database.as_deref(), Some("11111111-2222-4333-8444-000000000001"));
    }

    #[test]
    fn ids() {
        assert_eq!(page_id_from("https://www.notion.so/ws/Titulo-1a2b3c4d00004000800000000000abcd?pvs=4").as_deref(), Some("1a2b3c4d00004000800000000000abcd"));
        assert_eq!(page_id_from(PAGE_ID).as_deref(), Some("1a2b3c4d00004000800000000000abcd"));
        assert_eq!(page_id_from("https://exemplo.com/nada"), None);
        assert!(looks_like_id(ME));
        assert!(!looks_like_id("voce@exemplo.com"));
    }

    #[test]
    fn token_em_arquivo_600() {
        let dir = std::env::temp_dir().join(format!("aic-notion-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        // Só este teste mexe em AIC_CONFIG_DIR dentro do crate.
        unsafe { std::env::set_var("AIC_CONFIG_DIR", &dir) };
        std::fs::write(secrets_path(), "outra = \"chave\"\n").unwrap();
        save_token("ntn_teste").unwrap();
        let text = std::fs::read_to_string(secrets_path()).unwrap();
        assert!(text.contains("notion_token") && text.contains("outra"));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(std::fs::metadata(secrets_path()).unwrap().permissions().mode() & 0o777, 0o600);
        }
        if std::env::var("NOTION_TOKEN").is_err() {
            assert_eq!(load_token(), Some(("ntn_teste".into(), TokenSource::File)));
        }
        save_token("").unwrap();
        assert!(!std::fs::read_to_string(secrets_path()).unwrap().contains("notion_token"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
