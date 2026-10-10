//! Histórico de uso (tokens e custo) a partir dos transcripts do Claude Code.
//!
//! Os transcripts ficam em `~/.claude/projects/<cwd-codificado>/<sessão>.jsonl` (e os
//! subagentes em `<sessão>/subagents/agent-*.jsonl`). Cada resposta da API vira uma linha
//! `type: "assistant"` com `message.usage`, `message.model`, `message.id`, `requestId`,
//! `timestamp`, `sessionId` e `cwd`.
//!
//! Formato observado (Claude Code 2.1.x):
//! - a mesma resposta aparece em várias linhas (uma por bloco de conteúdo), com o mesmo
//!   `message.id` + `requestId`; durante o streaming o `output_tokens` cresce entre elas.
//!   Por isso a chave de deduplicação é `message.id:requestId` e fica o maior uso visto.
//! - forks de sessão copiam o histórico pra um arquivo novo: a deduplicação é global,
//!   e a resposta continua atribuída à sessão onde apareceu primeiro.
//! - `usage.cache_creation` separa a escrita de cache de 5 min e de 1 h (preços diferentes).
//! - `usage.speed == "fast"` é o modo rápido (preço premium).
//! - não há custo por linha nas versões atuais; versões antigas gravavam `costUSD`, que é
//!   usado quando aparece. As linhas `cost-state` trazem o custo do *processo* (zera a cada
//!   `--resume`), então não servem como total da sessão e não entram aqui.
//!
//! A leitura é incremental: guarda o offset de cada arquivo (como `events::EventTail`) e
//! só olha arquivos modificados dentro da janela (30 dias por padrão).

use std::collections::{BTreeMap, HashMap};
use std::io::{BufRead, BufReader, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use chrono::{DateTime, Local, NaiveDate};
use serde::{Deserialize, Serialize};

use crate::paths;

/// Janela padrão do histórico, em dias.
pub const WINDOW_DAYS: i64 = 30;

// ─── Tokens ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tokens {
    pub input: u64,
    pub output: u64,
    /// Escrita de cache (5 min + 1 h).
    pub cache_write: u64,
    pub cache_read: u64,
}

impl Tokens {
    pub fn total(&self) -> u64 {
        self.input + self.output + self.cache_write + self.cache_read
    }

    pub fn add(&mut self, o: &Tokens) {
        self.input += o.input;
        self.output += o.output;
        self.cache_write += o.cache_write;
        self.cache_read += o.cache_read;
    }
}

/// Uso de uma resposta, com o detalhe que o preço precisa.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct MsgUsage {
    pub tokens: Tokens,
    pub write_5m: u64,
    pub write_1h: u64,
    pub fast: bool,
}

// ─── Preços ──────────────────────────────────────────────────────────────────

/// Preço em USD por milhão de tokens.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Price {
    pub input: f64,
    pub output: f64,
    pub cache_write_5m: f64,
    pub cache_write_1h: f64,
    pub cache_read: f64,
}

impl Price {
    const fn new(input: f64, cache_write_5m: f64, cache_write_1h: f64, cache_read: f64, output: f64) -> Self {
        Self { input, output, cache_write_5m, cache_write_1h, cache_read }
    }

    fn scaled(&self, k: f64) -> Self {
        Self {
            input: self.input * k,
            output: self.output * k,
            cache_write_5m: self.cache_write_5m * k,
            cache_write_1h: self.cache_write_1h * k,
            cache_read: self.cache_read * k,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ModelPrice {
    pub price: Price,
    /// Preço acima de N tokens de prompt (entrada + cache), ex.: Haiku 5.5 acima de 100K.
    pub long: Option<(u64, Price)>,
    /// Multiplicador do modo rápido (`usage.speed == "fast"`), quando o modelo tem.
    pub fast: Option<f64>,
}

impl ModelPrice {
    fn flat(price: Price) -> Self {
        Self { price, long: None, fast: None }
    }

    pub fn cost(&self, u: &MsgUsage) -> f64 {
        let t = &u.tokens;
        let prompt = t.input + t.cache_write + t.cache_read;
        let mut p = match self.long {
            Some((limit, long)) if prompt > limit => long,
            _ => self.price,
        };
        if u.fast {
            p = p.scaled(self.fast.unwrap_or(1.0));
        }
        // escrita sem detalhe por TTL conta como 5 min
        let w5 = u.write_5m + t.cache_write.saturating_sub(u.write_5m + u.write_1h);
        (t.input as f64 * p.input
            + t.output as f64 * p.output
            + w5 as f64 * p.cache_write_5m
            + u.write_1h as f64 * p.cache_write_1h
            + t.cache_read as f64 * p.cache_read)
            / 1e6
    }
}

/// Tabela de preços por modelo (id normalizado, sem data).
///
/// Fonte: <https://platform.claude.com/docs/en/about-claude/pricing> (consultada em 2026-10-09).
/// Pode ser sobrescrita em `~/.config/aic/prices.toml`:
///
/// ```toml
/// [modelos."claude-opus-5-5"]
/// input = 4.0
/// output = 20.0
/// cache_write_5m = 5.0
/// cache_write_1h = 8.0
/// cache_read = 0.2
/// ```
#[derive(Debug, Clone)]
pub struct PriceTable {
    models: HashMap<String, ModelPrice>,
}

#[derive(Deserialize)]
struct PricesFile {
    #[serde(default)]
    modelos: HashMap<String, Price>,
}

impl Default for PriceTable {
    fn default() -> Self {
        Self::builtin()
    }
}

impl PriceTable {
    pub fn builtin() -> Self {
        let mut m = HashMap::new();
        let mut put = |ids: &[&str], mp: ModelPrice| {
            for id in ids {
                m.insert(id.to_string(), mp.clone());
            }
        };
        // entrada, escrita 5m, escrita 1h, leitura de cache, saída
        let fable51 = Price::new(10.0, 12.5, 20.0, 0.25, 50.0);
        let fable5 = Price::new(10.0, 12.5, 20.0, 1.0, 50.0);
        put(&["claude-fable-5-1", "claude-mythos-5-1"], ModelPrice::flat(fable51));
        put(&["claude-fable-5", "claude-mythos-5", "claude-mythos-preview"], ModelPrice::flat(fable5));
        put(&["claude-opus-5-5"], ModelPrice { fast: Some(2.0), ..ModelPrice::flat(Price::new(4.0, 5.0, 8.0, 0.20, 20.0)) });
        let opus = Price::new(5.0, 6.25, 10.0, 0.50, 25.0);
        put(&["claude-opus-5", "claude-opus-4-8"], ModelPrice { fast: Some(2.0), ..ModelPrice::flat(opus) });
        put(&["claude-opus-4-7", "claude-opus-4-6", "claude-opus-4-5"], ModelPrice::flat(opus));
        put(&["claude-opus-4-1", "claude-opus-4"], ModelPrice::flat(Price::new(15.0, 18.75, 30.0, 1.50, 75.0)));
        put(&["claude-sonnet-5-5"], ModelPrice::flat(Price::new(2.0, 2.5, 4.0, 0.10, 10.0)));
        put(&["claude-sonnet-5"], ModelPrice::flat(Price::new(2.0, 2.5, 4.0, 0.20, 10.0)));
        put(&["claude-sonnet-4-6", "claude-sonnet-4-5", "claude-sonnet-4"], ModelPrice::flat(Price::new(3.0, 3.75, 6.0, 0.30, 15.0)));
        put(
            &["claude-haiku-5-5"],
            ModelPrice { long: Some((100_000, Price::new(0.50, 0.625, 1.0, 0.05, 2.50))), ..ModelPrice::flat(Price::new(0.10, 0.125, 0.20, 0.01, 0.50)) },
        );
        put(&["claude-haiku-4-5"], ModelPrice::flat(Price::new(1.0, 1.25, 2.0, 0.10, 5.0)));
        put(&["claude-3-5-haiku"], ModelPrice::flat(Price::new(0.80, 1.0, 1.6, 0.08, 4.0)));
        Self { models: m }
    }

    /// Tabela embutida + `prices.toml` do usuário (se existir e for válido).
    pub fn load() -> Self {
        let mut t = Self::builtin();
        if let Ok(text) = std::fs::read_to_string(paths::config_dir().join("prices.toml")) {
            t.apply_overrides(&text);
        }
        t
    }

    pub fn apply_overrides(&mut self, toml_text: &str) {
        if let Ok(f) = toml::from_str::<PricesFile>(toml_text) {
            for (id, p) in f.modelos {
                let id = normalize_model(&id);
                let entry = self.models.entry(id).or_insert_with(|| ModelPrice::flat(p));
                // sobrescrever cancela a faixa de contexto longo embutida
                *entry = ModelPrice { price: p, long: None, fast: entry.fast };
            }
        }
    }

    pub fn get(&self, model: &str) -> Option<&ModelPrice> {
        self.models.get(&normalize_model(model))
    }

    pub fn cost(&self, model: &str, u: &MsgUsage) -> Option<f64> {
        self.get(model).map(|p| p.cost(u))
    }
}

/// `claude-haiku-4-5-20251001` → `claude-haiku-4-5`; tira `[1m]`, prefixo `anthropic.` e `@data`.
pub fn normalize_model(id: &str) -> String {
    let mut s = id.trim().to_ascii_lowercase();
    if let Some(i) = s.find('[') {
        s.truncate(i);
    }
    if let Some(rest) = s.strip_prefix("anthropic.") {
        s = rest.to_string();
    }
    if let Some(i) = s.find('@') {
        s.truncate(i);
    }
    if let Some(i) = s.rfind('-') {
        let tail = &s[i + 1..];
        if tail.len() == 8 && tail.chars().all(|c| c.is_ascii_digit()) {
            s.truncate(i);
        }
    }
    // `-v1:0` do Bedrock
    if let Some(i) = s.find("-v1") {
        s.truncate(i);
    }
    s
}

/// `claude-opus-5-5` → `Opus 5.5`; `claude-haiku-4-5-20251001` → `Haiku 4.5`.
pub fn model_label(id: &str) -> String {
    let n = normalize_model(id);
    let rest = n.strip_prefix("claude-").unwrap_or(&n);
    let mut parts = rest.split('-');
    let Some(first) = parts.next() else { return id.to_string() };
    let (family, version): (String, Vec<&str>) = if first.chars().all(|c| c.is_ascii_digit()) {
        // ids antigos: claude-3-5-haiku
        let nums: Vec<&str> = std::iter::once(first).chain(parts.clone().take_while(|p| p.chars().all(|c| c.is_ascii_digit()))).collect();
        let fam = parts.find(|p| !p.chars().all(|c| c.is_ascii_digit())).unwrap_or("");
        (fam.to_string(), nums)
    } else {
        (first.to_string(), parts.collect())
    };
    let mut fam = family;
    if let Some(c) = fam.get_mut(0..1) {
        c.make_ascii_uppercase();
    }
    if version.is_empty() { fam } else { format!("{fam} {}", version.join(".")) }
}

/// Família do modelo, pra ordenar e escolher cor (fable > opus > sonnet > haiku > outros).
pub fn model_rank(id: &str) -> u8 {
    let n = normalize_model(id);
    if n.contains("fable") || n.contains("mythos") {
        0
    } else if n.contains("opus") {
        1
    } else if n.contains("sonnet") {
        2
    } else if n.contains("haiku") {
        3
    } else {
        4
    }
}

// ─── Leitura dos transcripts ─────────────────────────────────────────────────

#[derive(Deserialize)]
struct Line {
    timestamp: Option<String>,
    #[serde(rename = "sessionId")]
    session_id: Option<String>,
    cwd: Option<String>,
    #[serde(rename = "requestId")]
    request_id: Option<String>,
    uuid: Option<String>,
    #[serde(rename = "costUSD")]
    cost_usd: Option<f64>,
    message: Option<Message>,
}

#[derive(Deserialize)]
struct Message {
    id: Option<String>,
    model: Option<String>,
    usage: Option<RawUsage>,
}

#[derive(Deserialize, Default)]
struct RawUsage {
    #[serde(default)]
    input_tokens: u64,
    #[serde(default)]
    output_tokens: u64,
    #[serde(default)]
    cache_creation_input_tokens: u64,
    #[serde(default)]
    cache_read_input_tokens: u64,
    #[serde(default)]
    cache_creation: Option<CacheCreation>,
    #[serde(default)]
    speed: Option<String>,
}

#[derive(Deserialize, Default)]
struct CacheCreation {
    #[serde(default)]
    ephemeral_5m_input_tokens: u64,
    #[serde(default)]
    ephemeral_1h_input_tokens: u64,
}

/// Uma resposta da API, já deduplicada.
#[derive(Debug, Clone)]
struct Entry {
    ts_ms: i64,
    day: NaiveDate,
    model: u32,
    session: u32,
    project: u32,
    tokens: Tokens,
    cost: f64,
    priced: bool,
}

/// Strings repetidas (modelo, sessão, projeto) guardadas uma vez só.
#[derive(Default)]
struct Interner {
    ids: HashMap<String, u32>,
    names: Vec<String>,
}

impl Interner {
    fn id(&mut self, s: &str) -> u32 {
        if let Some(&i) = self.ids.get(s) {
            return i;
        }
        let i = self.names.len() as u32;
        self.names.push(s.to_string());
        self.ids.insert(s.to_string(), i);
        i
    }

    fn name(&self, i: u32) -> &str {
        &self.names[i as usize]
    }
}

/// Contadores da última varredura (pra mostrar e pra testar a incrementalidade).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ScanStats {
    pub files: usize,
    pub bytes_read: u64,
    pub lines: u64,
}

/// Agrega os transcripts incrementalmente. Roda numa thread do app; não é `Sync`.
pub struct Aggregator {
    root: PathBuf,
    days: i64,
    prices: PriceTable,
    offsets: HashMap<PathBuf, u64>,
    entries: HashMap<String, Entry>,
    strings: Interner,
    pub last_scan: ScanStats,
}

impl Aggregator {
    /// Lê `<CLAUDE_CONFIG_DIR ou ~/.claude>/projects`.
    pub fn new(prices: PriceTable) -> Self {
        Self::with_root(paths::claude_dir().join("projects"), prices)
    }

    pub fn with_root(root: PathBuf, prices: PriceTable) -> Self {
        Self { root, days: WINDOW_DAYS, prices, offsets: HashMap::new(), entries: HashMap::new(), strings: Interner::default(), last_scan: ScanStats::default() }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Lê o que há de novo nos transcripts modificados dentro da janela.
    pub fn scan(&mut self) -> ScanStats {
        let mut stats = ScanStats::default();
        let cutoff = SystemTime::now() - Duration::from_secs((self.days.max(1) as u64 + 1) * 86_400);
        let mut files = Vec::new();
        collect_jsonl(&self.root, 0, &mut files);
        for (path, len, mtime) in files {
            if mtime < cutoff {
                continue;
            }
            stats.files += 1;
            let off = self.offsets.get(&path).copied().unwrap_or(0);
            let off = if len < off { 0 } else { off }; // truncado/recriado
            if len == off {
                continue;
            }
            let (consumed, lines) = self.read_file(&path, off);
            stats.bytes_read += consumed;
            stats.lines += lines;
            self.offsets.insert(path, off + consumed);
        }
        self.prune();
        self.last_scan = stats;
        stats
    }

    /// Lê linhas completas a partir de `off`. Devolve (bytes consumidos, linhas lidas).
    fn read_file(&mut self, path: &Path, off: u64) -> (u64, u64) {
        let Ok(mut f) = std::fs::File::open(path) else { return (0, 0) };
        if f.seek(SeekFrom::Start(off)).is_err() {
            return (0, 0);
        }
        // sessão/projeto de reserva quando a linha não traz: pelo caminho do arquivo
        let file_session = fallback_session(path);
        let file_project = path
            .strip_prefix(&self.root)
            .ok()
            .and_then(|p| p.components().next())
            .map(|c| c.as_os_str().to_string_lossy().into_owned())
            .unwrap_or_default();
        let mut reader = BufReader::with_capacity(256 * 1024, f);
        let mut buf = Vec::with_capacity(64 * 1024);
        let (mut consumed, mut lines) = (0u64, 0u64);
        loop {
            buf.clear();
            let Ok(n) = reader.read_until(b'\n', &mut buf) else { break };
            if n == 0 || buf.last() != Some(&b'\n') {
                break; // fim, ou linha ainda sendo escrita
            }
            consumed += n as u64;
            lines += 1;
            // atalho barato: a maioria das linhas (prompts, resultados de ferramenta) não tem uso
            if !contains(&buf, b"\"usage\"") {
                continue;
            }
            let Ok(line) = serde_json::from_slice::<Line>(&buf) else { continue };
            self.ingest(line, &file_session, &file_project, path, off + consumed);
        }
        (consumed, lines)
    }

    fn ingest(&mut self, line: Line, file_session: &str, file_project: &str, path: &Path, at: u64) {
        let Some(msg) = line.message else { return };
        let Some(raw) = msg.usage else { return };
        let model = msg.model.unwrap_or_default();
        if model.is_empty() || model.starts_with('<') {
            return; // `<synthetic>`: mensagens geradas localmente, sem chamada à API
        }
        let Some(ts) = line.timestamp.as_deref().and_then(|t| DateTime::parse_from_rfc3339(t).ok()) else { return };
        let cc = raw.cache_creation.unwrap_or_default();
        let usage = MsgUsage {
            tokens: Tokens { input: raw.input_tokens, output: raw.output_tokens, cache_write: raw.cache_creation_input_tokens, cache_read: raw.cache_read_input_tokens },
            write_5m: cc.ephemeral_5m_input_tokens,
            write_1h: cc.ephemeral_1h_input_tokens,
            fast: raw.speed.as_deref() == Some("fast"),
        };
        let (cost, priced) = match (line.cost_usd, self.prices.cost(&model, &usage)) {
            (Some(c), _) => (c, true),
            (None, Some(c)) => (c, true),
            (None, None) => (0.0, false),
        };
        let key = match (&msg.id, &line.request_id) {
            (Some(id), Some(req)) => format!("{id}:{req}"),
            (Some(id), None) => id.clone(),
            _ => line.uuid.clone().unwrap_or_else(|| format!("{}#{at}", path.display())),
        };
        if let Some(e) = self.entries.get_mut(&key) {
            // mesma resposta em outra linha: fica o maior uso (o streaming só cresce)
            if usage.tokens.total() >= e.tokens.total() {
                e.tokens = usage.tokens;
                e.cost = cost;
                e.priced = priced;
            }
            return;
        }
        let session = line.session_id.unwrap_or_else(|| file_session.to_string());
        let project = line.cwd.unwrap_or_else(|| file_project.to_string());
        let entry = Entry {
            ts_ms: ts.timestamp_millis(),
            day: ts.with_timezone(&Local).date_naive(),
            model: self.strings.id(&model),
            session: self.strings.id(&session),
            project: self.strings.id(&project),
            tokens: usage.tokens,
            cost,
            priced,
        };
        self.entries.insert(key, entry);
    }

    /// Esquece respostas fora da janela (mantém a memória limitada).
    fn prune(&mut self) {
        let min = Local::now().date_naive() - chrono::Duration::days(self.days + 1);
        self.entries.retain(|_, e| e.day >= min);
    }

    /// Consolida por (dia, modelo, sessão, projeto).
    pub fn report(&self, today: NaiveDate) -> Report {
        let min = today - chrono::Duration::days(self.days - 1);
        let mut rows: HashMap<(NaiveDate, u32, u32, u32), Row> = HashMap::new();
        let mut unpriced: BTreeMap<String, u64> = BTreeMap::new();
        for e in self.entries.values().filter(|e| e.day >= min && e.day <= today) {
            let r = rows.entry((e.day, e.model, e.session, e.project)).or_insert_with(|| Row {
                day: e.day,
                model: self.strings.name(e.model).to_string(),
                session: self.strings.name(e.session).to_string(),
                project: self.strings.name(e.project).to_string(),
                ..Default::default()
            });
            r.tokens.add(&e.tokens);
            r.cost += e.cost;
            r.messages += 1;
            r.last_ts_ms = r.last_ts_ms.max(e.ts_ms);
            if !e.priced {
                *unpriced.entry(self.strings.name(e.model).to_string()).or_default() += e.tokens.total();
            }
        }
        let mut rows: Vec<Row> = rows.into_values().collect();
        rows.sort_by(|a, b| a.day.cmp(&b.day).then_with(|| a.model.cmp(&b.model)).then_with(|| a.session.cmp(&b.session)));
        Report { today, window_days: self.days, rows, unpriced: unpriced.into_iter().collect(), scan: self.last_scan, generated_ms: chrono::Utc::now().timestamp_millis() }
    }
}

/// `.../<sessão>.jsonl` → sessão; `.../<sessão>/subagents/agent-x.jsonl` → sessão.
fn fallback_session(path: &Path) -> String {
    let parent = path.parent();
    if parent.and_then(|p| p.file_name()).is_some_and(|n| n == "subagents")
        && let Some(s) = parent.and_then(Path::parent).and_then(Path::file_name)
    {
        return s.to_string_lossy().into_owned();
    }
    path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default()
}

/// `projects/<proj>/*.jsonl` e `projects/<proj>/<sessão>/subagents/*.jsonl`.
fn collect_jsonl(dir: &Path, depth: u8, out: &mut Vec<(PathBuf, u64, SystemTime)>) {
    if depth > 3 {
        return;
    }
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let Ok(ft) = e.file_type() else { continue };
        let path = e.path();
        if ft.is_dir() {
            collect_jsonl(&path, depth + 1, out);
        } else if path.extension().is_some_and(|x| x == "jsonl")
            && let Ok(m) = e.metadata()
        {
            out.push((path, m.len(), m.modified().unwrap_or(SystemTime::UNIX_EPOCH)));
        }
    }
}

fn contains(hay: &[u8], needle: &[u8]) -> bool {
    hay.windows(needle.len()).any(|w| w == needle)
}

// ─── Relatório ───────────────────────────────────────────────────────────────

/// Uso consolidado por (dia, modelo, sessão, projeto).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Row {
    pub day: NaiveDate,
    pub model: String,
    pub session: String,
    /// cwd da sessão.
    pub project: String,
    pub tokens: Tokens,
    pub cost: f64,
    pub messages: u32,
    pub last_ts_ms: i64,
}

/// Soma de um grupo (modelo, sessão, projeto ou período).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Bucket {
    pub key: String,
    /// Projeto (cwd) mais recente do grupo.
    pub project: String,
    pub tokens: Tokens,
    pub cost: f64,
    pub messages: u32,
    pub last_ts_ms: i64,
}

impl Bucket {
    fn add(&mut self, r: &Row) {
        self.tokens.add(&r.tokens);
        self.cost += r.cost;
        self.messages += r.messages;
        if r.last_ts_ms >= self.last_ts_ms {
            self.last_ts_ms = r.last_ts_ms;
            self.project = r.project.clone();
        }
    }
}

/// Um dia do gráfico: custo e tokens por modelo.
#[derive(Debug, Clone, PartialEq)]
pub struct Day {
    pub day: NaiveDate,
    pub models: Vec<Bucket>,
}

impl Day {
    pub fn cost(&self) -> f64 {
        self.models.iter().map(|m| m.cost).sum()
    }

    pub fn tokens(&self) -> Tokens {
        let mut t = Tokens::default();
        for m in &self.models {
            t.add(&m.tokens);
        }
        t
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Report {
    pub today: NaiveDate,
    pub window_days: i64,
    pub rows: Vec<Row>,
    /// Modelos sem preço conhecido (e quantos tokens tiveram): custo não estimado.
    pub unpriced: Vec<(String, u64)>,
    pub scan: ScanStats,
    pub generated_ms: i64,
}

impl Report {
    /// Primeiro dia de um período de `days` dias terminando hoje (1 = só hoje).
    pub fn since(&self, days: i64) -> NaiveDate {
        self.today - chrono::Duration::days(days.max(1) - 1)
    }

    fn rows_in(&self, days: i64) -> impl Iterator<Item = &Row> {
        let since = self.since(days);
        self.rows.iter().filter(move |r| r.day >= since)
    }

    pub fn totals(&self, days: i64) -> Bucket {
        let mut b = Bucket::default();
        for r in self.rows_in(days) {
            b.add(r);
        }
        b
    }

    fn group(&self, days: i64, key: impl Fn(&Row) -> &str) -> Vec<Bucket> {
        let mut map: HashMap<&str, Bucket> = HashMap::new();
        for r in self.rows_in(days) {
            let k = key(r);
            map.entry(k).or_insert_with(|| Bucket { key: k.to_string(), ..Default::default() }).add(r);
        }
        let mut v: Vec<Bucket> = map.into_values().collect();
        v.sort_by(|a, b| b.cost.total_cmp(&a.cost).then_with(|| b.tokens.total().cmp(&a.tokens.total())).then_with(|| a.key.cmp(&b.key)));
        v
    }

    pub fn by_model(&self, days: i64) -> Vec<Bucket> {
        self.group(days, |r| &r.model)
    }

    pub fn by_session(&self, days: i64) -> Vec<Bucket> {
        self.group(days, |r| &r.session)
    }

    pub fn by_project(&self, days: i64) -> Vec<Bucket> {
        self.group(days, |r| &r.project)
    }

    /// Os últimos `days` dias, em ordem, incluindo dias sem uso.
    pub fn daily(&self, days: i64) -> Vec<Day> {
        let since = self.since(days);
        let mut out: Vec<Day> = (0..days.max(1)).map(|i| Day { day: since + chrono::Duration::days(i), models: vec![] }).collect();
        for r in self.rows_in(days) {
            let i = (r.day - since).num_days() as usize;
            let Some(d) = out.get_mut(i) else { continue };
            match d.models.iter_mut().find(|m| m.key == r.model) {
                Some(m) => m.add(r),
                None => {
                    let mut b = Bucket { key: r.model.clone(), ..Default::default() };
                    b.add(r);
                    d.models.push(b);
                }
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// Linha sintética de transcript, só com os campos de uso.
    fn line(id: &str, req: &str, model: &str, ts: &str, session: &str, cwd: &str, usage: serde_json::Value) -> String {
        let v = json!({
            "type": "assistant", "timestamp": ts, "sessionId": session, "cwd": cwd, "requestId": req,
            "uuid": format!("u-{id}-{req}-{}", usage["output_tokens"]),
            "message": {"id": id, "model": model, "role": "assistant", "content": [], "usage": usage}
        });
        format!("{v}\n")
    }

    fn usage(input: u64, w5: u64, w1h: u64, read: u64, out: u64) -> serde_json::Value {
        json!({
            "input_tokens": input, "output_tokens": out,
            "cache_creation_input_tokens": w5 + w1h, "cache_read_input_tokens": read,
            "cache_creation": {"ephemeral_5m_input_tokens": w5, "ephemeral_1h_input_tokens": w1h},
            "service_tier": "standard", "speed": "standard"
        })
    }

    fn tmp() -> PathBuf {
        let d = std::env::temp_dir().join(format!("aic-usage-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn today_ts() -> (NaiveDate, String) {
        let now = Local::now();
        (now.date_naive(), now.to_rfc3339())
    }

    #[test]
    fn precos_e_custo() {
        let t = PriceTable::builtin();
        // Opus 5.5: 1M de cada tipo
        let u = MsgUsage { tokens: Tokens { input: 1_000_000, output: 1_000_000, cache_write: 2_000_000, cache_read: 1_000_000 }, write_5m: 1_000_000, write_1h: 1_000_000, fast: false };
        let c = t.cost("claude-opus-5-5", &u).unwrap();
        assert!((c - (4.0 + 20.0 + 5.0 + 8.0 + 0.2)).abs() < 1e-9, "{c}");
        // modo rápido = 2x
        let fast = t.cost("claude-opus-5-5", &MsgUsage { fast: true, ..u }).unwrap();
        assert!((fast - 2.0 * c).abs() < 1e-9);
        // data no id e [1m]
        assert!(t.get("claude-haiku-4-5-20251001").is_some());
        assert!(t.get("claude-sonnet-5[1m]").is_some());
        assert!(t.get("claude-desconhecido-9").is_none());
        // Haiku 5.5 acima de 100K de prompt muda de faixa
        let small = MsgUsage { tokens: Tokens { input: 50_000, ..Default::default() }, ..Default::default() };
        let big = MsgUsage { tokens: Tokens { input: 200_000, ..Default::default() }, ..Default::default() };
        assert!((t.cost("claude-haiku-5-5", &small).unwrap() - 0.005).abs() < 1e-12);
        assert!((t.cost("claude-haiku-5-5", &big).unwrap() - 0.1).abs() < 1e-12);
        // escrita sem detalhe de TTL conta como 5 min
        let legacy = MsgUsage { tokens: Tokens { cache_write: 1_000_000, ..Default::default() }, ..Default::default() };
        assert!((t.cost("claude-sonnet-5", &legacy).unwrap() - 2.5).abs() < 1e-9);
    }

    #[test]
    fn sobrescrita_de_precos() {
        let mut t = PriceTable::builtin();
        t.apply_overrides("[modelos.\"claude-novo-1\"]\ninput = 1.0\noutput = 2.0\ncache_write_5m = 0\ncache_write_1h = 0\ncache_read = 0\n");
        let u = MsgUsage { tokens: Tokens { input: 1_000_000, output: 1_000_000, ..Default::default() }, ..Default::default() };
        assert!((t.cost("claude-novo-1", &u).unwrap() - 3.0).abs() < 1e-9);
    }

    #[test]
    fn nomes_de_modelo() {
        assert_eq!(model_label("claude-opus-5-5"), "Opus 5.5");
        assert_eq!(model_label("claude-haiku-4-5-20251001"), "Haiku 4.5");
        assert_eq!(model_label("claude-sonnet-5"), "Sonnet 5");
        assert_eq!(model_label("claude-fable-5-1"), "Fable 5.1");
        assert_eq!(model_label("claude-3-5-haiku-20241022"), "Haiku 3.5");
        assert!(model_rank("claude-fable-5-1") < model_rank("claude-opus-5-5"));
        assert!(model_rank("claude-sonnet-5") < model_rank("claude-haiku-5-5"));
    }

    #[test]
    fn deduplica_streaming_e_le_incremental() {
        let root = tmp();
        let proj = root.join("-home-x-repo");
        std::fs::create_dir_all(proj.join("s1/subagents")).unwrap();
        let (today, ts) = today_ts();
        let file = proj.join("s1.jsonl");
        // mesma resposta em 3 linhas (blocos), output crescendo; + uma linha sem uso
        let mut text = String::new();
        text += "{\"type\":\"user\",\"message\":{\"role\":\"user\",\"content\":\"oi\"}}\n";
        text += &line("msg_a", "req_a", "claude-opus-5-5", &ts, "s1", "/home/x/repo", usage(10, 1000, 0, 0, 5));
        text += &line("msg_a", "req_a", "claude-opus-5-5", &ts, "s1", "/home/x/repo", usage(10, 1000, 0, 0, 50));
        text += &line("msg_a", "req_a", "claude-opus-5-5", &ts, "s1", "/home/x/repo", usage(10, 1000, 0, 0, 50));
        // linha incompleta (ainda sendo escrita) fica pra depois
        let partial = line("msg_b", "req_b", "claude-sonnet-5", &ts, "s1", "/home/x/repo", usage(1, 0, 0, 500, 7));
        std::fs::write(&file, format!("{text}{}", partial.trim_end())).unwrap();

        let mut agg = Aggregator::with_root(root.clone(), PriceTable::builtin());
        agg.scan();
        let r = agg.report(today);
        let tot = r.totals(1);
        assert_eq!(tot.messages, 1);
        assert_eq!(tot.tokens, Tokens { input: 10, output: 50, cache_write: 1000, cache_read: 0 });
        let expected = (10.0 * 4.0 + 50.0 * 20.0 + 1000.0 * 5.0) / 1e6;
        assert!((tot.cost - expected).abs() < 1e-12);

        // completa a linha + subagente (sessionId do pai) + fork copiando msg_a
        std::fs::write(&file, format!("{text}{partial}")).unwrap();
        std::fs::write(proj.join("s1/subagents/agent-1.jsonl"), line("msg_c", "req_c", "claude-haiku-5-5", &ts, "s1", "/home/x/repo", usage(100, 0, 0, 0, 10))).unwrap();
        std::fs::write(proj.join("s2.jsonl"), line("msg_a", "req_a", "claude-opus-5-5", &ts, "s2", "/home/x/repo", usage(10, 1000, 0, 0, 50))).unwrap();
        let before = agg.last_scan;
        let stats = agg.scan();
        assert!(stats.bytes_read > 0 && stats.bytes_read < before.bytes_read + 2000);
        let r = agg.report(today);
        assert_eq!(r.totals(1).messages, 3);
        let sessions = r.by_session(30);
        assert_eq!(sessions.len(), 1, "fork não duplica: {sessions:?}");
        assert_eq!(sessions[0].key, "s1");
        assert_eq!(sessions[0].project, "/home/x/repo");
        let models: Vec<String> = r.by_model(30).into_iter().map(|b| b.key).collect();
        assert_eq!(models.len(), 3);
        assert_eq!(models[0], "claude-opus-5-5");
        // nada novo: não relê
        assert_eq!(agg.scan().bytes_read, 0);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn por_dia_e_custo_informado() {
        let root = tmp();
        let proj = root.join("p");
        std::fs::create_dir_all(&proj).unwrap();
        let today = Local::now().date_naive();
        let at = |d: i64| (today - chrono::Duration::days(d)).and_hms_opt(12, 0, 0).unwrap().and_local_timezone(Local).single().unwrap().to_rfc3339();
        let mut text = String::new();
        text += &line("m1", "r1", "claude-sonnet-5", &at(0), "s", "/p", usage(1000, 0, 0, 0, 0));
        text += &line("m2", "r2", "claude-sonnet-5", &at(3), "s", "/p", usage(1000, 0, 0, 0, 0));
        text += &line("m3", "r3", "claude-opus-5", &at(3), "s", "/p", usage(1000, 0, 0, 0, 0));
        text += &line("m4", "r4", "claude-opus-5", &at(45), "s", "/p", usage(1000, 0, 0, 0, 0)); // fora da janela
        text += &line("m5", "r5", "modelo-x", &at(1), "s", "/p", usage(7, 0, 0, 0, 0)); // sem preço
        // custo informado na linha (formato antigo) tem prioridade
        let mut legacy: serde_json::Value = serde_json::from_str(&line("m6", "r6", "claude-sonnet-5", &at(2), "s", "/p", usage(1, 0, 0, 0, 0))).unwrap();
        legacy["costUSD"] = json!(1.5);
        text += &format!("{legacy}\n");
        std::fs::write(proj.join("s.jsonl"), text).unwrap();
        let mut agg = Aggregator::with_root(root.clone(), PriceTable::builtin());
        agg.scan();
        let r = agg.report(today);
        let days = r.daily(7);
        assert_eq!(days.len(), 7);
        assert_eq!(days.last().unwrap().day, today);
        assert_eq!(days[6].models.len(), 1);
        assert_eq!(days[3].models.len(), 2);
        assert!((days[4].cost() - 1.5).abs() < 1e-12);
        assert_eq!(r.totals(30).messages, 5);
        assert_eq!(r.unpriced, vec![("modelo-x".to_string(), 7)]);
        assert_eq!(r.totals(1).messages, 1);
        let _ = std::fs::remove_dir_all(root);
    }

    /// Mede a varredura nos transcripts de verdade: `cargo test -p aic-core --release varredura_real -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn varredura_real() {
        let mut agg = Aggregator::new(PriceTable::load());
        let t = std::time::Instant::now();
        let s = agg.scan();
        let first = t.elapsed();
        let t = std::time::Instant::now();
        let again = agg.scan();
        let r = agg.report(Local::now().date_naive());
        let tot = r.totals(30);
        eprintln!("1ª: {first:?} {s:?} | incremental: {:?} {again:?} | 30d: {} respostas ${:.2} | sem preço: {:?}", t.elapsed(), tot.messages, tot.cost, r.unpriced);
    }
}
