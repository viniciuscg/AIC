//! Histórico de uso (tokens/custo por dia, modelo, sessão) a partir dos transcripts.
//!
//! Uma thread em segundo plano lê `~/.claude/projects/**.jsonl` com o
//! [`Aggregator`](aic_core::usage::Aggregator): varredura completa (30 dias) quando a aba
//! Tokens abre pela primeira vez e incremental a cada 30 s depois disso. O relatório chega por
//! canal; a UI só desenha.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::time::{Duration, Instant};

use chrono::{Datelike, Local, NaiveDate};
use egui::{Align2, Color32, CornerRadius, FontId, Rect, RichText, Sense, Stroke, Ui, pos2, vec2};
use aic_core::usage::{self, Aggregator, Bucket, PriceTable, Report, Tokens};
use aic_core::{Id, paths};

use crate::app::App;
use crate::theme::Theme;
use crate::ui::widgets::{self, I_COST};

/// Intervalo da leitura incremental.
const EVERY: Duration = Duration::from_secs(30);
/// Quantos modelos ganham cor própria; o resto vira "Outros".
const MAX_SERIES: usize = 6;
const PERIODS: [(i64, &str); 3] = [(1, "Hoje"), (7, "7 dias"), (30, "30 dias")];

pub struct UsageHistory {
    rx: Option<Receiver<Report>>,
    cmd: Option<Sender<()>>,
    pub report: Option<Arc<Report>>,
    /// Período das listas (1, 7 ou 30 dias).
    pub period: i64,
    received: Option<Instant>,
}

impl Default for UsageHistory {
    fn default() -> Self {
        Self { rx: None, cmd: None, report: None, period: 7, received: None }
    }
}

impl UsageHistory {
    /// Sobe a thread de leitura na primeira vez (a aba Tokens chama a cada frame). Antes disso
    /// nada lê os transcripts.
    pub fn ensure_started(&mut self, ctx: &egui::Context) {
        if self.rx.is_none() {
            let period = self.period;
            *self = Self { period, ..Self::start(ctx) };
        }
    }

    /// Sobe a thread de leitura. Caminhos e preços são resolvidos aqui (thread da UI),
    /// assim a thread não depende de variáveis de ambiente mudando depois.
    pub fn start(ctx: &egui::Context) -> Self {
        let root = paths::claude_dir().join("projects");
        let prices = PriceTable::load();
        let (tx, rx) = mpsc::channel();
        let (cmd_tx, cmd_rx) = mpsc::channel::<()>();
        let ctx = ctx.clone();
        let spawned = std::thread::Builder::new().name("aic-usage".into()).spawn(move || {
            let mut agg = Aggregator::with_root(root, prices);
            loop {
                agg.scan();
                if tx.send(agg.report(Local::now().date_naive())).is_err() {
                    break;
                }
                ctx.request_repaint();
                match cmd_rx.recv_timeout(EVERY) {
                    Ok(()) | Err(RecvTimeoutError::Timeout) => {}
                    Err(RecvTimeoutError::Disconnected) => break,
                }
            }
        });
        match spawned {
            Ok(_) => Self { rx: Some(rx), cmd: Some(cmd_tx), ..Default::default() },
            Err(_) => Self::default(),
        }
    }

    /// Pega o relatório mais novo, se chegou (chamado todo frame; não bloqueia).
    pub fn poll(&mut self) {
        let Some(rx) = &self.rx else { return };
        let mut latest = None;
        while let Ok(r) = rx.try_recv() {
            latest = Some(r);
        }
        if let Some(r) = latest {
            self.report = Some(Arc::new(r));
            self.received = Some(Instant::now());
        }
    }

    /// Pede uma leitura agora (sem esperar os 30 s).
    pub fn refresh(&self) {
        if let Some(c) = &self.cmd {
            let _ = c.send(());
        }
    }

    pub fn today_cost(&self) -> Option<f64> {
        self.report.as_ref().map(|r| r.totals(1).cost).filter(|c| *c > 0.0)
    }
}

// ─── Formatação ──────────────────────────────────────────────────────────────

pub fn fmt_tokens(n: u64) -> String {
    let f = n as f64;
    match n {
        0..1_000 => n.to_string(),
        1_000..100_000 => format!("{:.1}K", f / 1e3),
        100_000..1_000_000 => format!("{:.0}K", f / 1e3),
        1_000_000..100_000_000 => format!("{:.1}M", f / 1e6),
        100_000_000..1_000_000_000 => format!("{:.0}M", f / 1e6),
        _ => format!("{:.1}B", f / 1e9),
    }
}

pub fn fmt_usd(x: f64) -> String {
    if x <= 0.0 {
        "$0".into()
    } else if x < 0.01 {
        "<$0.01".into()
    } else if x < 10_000.0 {
        format!("${x:.2}")
    } else {
        format!("${:.1}K", x / 1e3)
    }
}

fn weekday(d: NaiveDate) -> &'static str {
    ["seg", "ter", "qua", "qui", "sex", "sáb", "dom"][d.weekday().num_days_from_monday() as usize]
}

fn ago(ts_ms: i64) -> String {
    let s = ((chrono::Utc::now().timestamp_millis() - ts_ms) / 1000).max(0);
    if s < 60 {
        "agora".into()
    } else if s < 3600 {
        format!("há {}min", s / 60)
    } else if s < 86_400 {
        format!("há {}h", s / 3600)
    } else {
        format!("há {}d", s / 86_400)
    }
}

/// Passo "redondo" (1, 2, 5 × 10^n) pra ~4 linhas de grade: (passo, nº de faixas).
fn nice_axis(max: f64) -> (f64, usize) {
    if max <= 0.0 {
        return (0.25, 4);
    }
    let raw = max / 4.0;
    let e = 10f64.powf(raw.log10().floor());
    let step = [1.0, 2.0, 5.0, 10.0].into_iter().map(|m| m * e).find(|s| *s >= raw).unwrap_or(10.0 * e);
    (step, (max / step).ceil().max(1.0) as usize)
}

// ─── Cores por modelo ────────────────────────────────────────────────────────

/// Cor fixa por modelo: ordem por família/versão nos 30 dias (não pelo ranking do
/// período), então trocar o período não repinta ninguém.
struct Palette {
    order: Vec<String>,
    colors: HashMap<String, Color32>,
    other: Color32,
}

impl Palette {
    fn new(report: &Report, theme: &Theme) -> Self {
        let mut order: Vec<String> = report.by_model(report.window_days).into_iter().map(|b| b.key).collect();
        order.sort_by(|a, b| usage::model_rank(a).cmp(&usage::model_rank(b)).then_with(|| usage::normalize_model(b).cmp(&usage::normalize_model(a))));
        let colors = order.iter().take(MAX_SERIES).enumerate().map(|(i, m)| (m.clone(), theme.agent(i as u8))).collect();
        Self { order, colors, other: theme.faint }
    }

    fn color(&self, model: &str) -> Color32 {
        self.colors.get(model).copied().unwrap_or(self.other)
    }

    /// Posição na pilha (de baixo pra cima); modelos sem cor vão juntos no topo.
    fn slot(&self, model: &str) -> usize {
        self.order.iter().position(|m| m == model).unwrap_or(usize::MAX).min(MAX_SERIES)
    }

    fn label(&self, model: &str) -> String {
        if self.colors.contains_key(model) { usage::model_label(model) } else { "Outros".into() }
    }
}

// ─── Tela ────────────────────────────────────────────────────────────────────

fn section(ui: &mut Ui, text: &str, theme: &Theme) {
    ui.label(RichText::new(text).font(FontId::monospace(11.0)).color(theme.dim));
}

pub fn show(app: &mut App, ui: &mut Ui) {
    app.usage_history.poll();
    let theme = app.theme.clone();
    ui.add_space(26.0);
    let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 1.0), Sense::hover());
    ui.painter().hline(r.x_range(), r.center().y, Stroke::new(1.0, theme.border));
    ui.add_space(14.0);

    ui.horizontal(|ui| {
        widgets::heading(ui, &format!("{I_COST}  Histórico de uso"), &theme);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.button(RichText::new(format!("{}  Atualizar", widgets::I_RESTART)).color(theme.dim)).on_hover_text("Lê os transcripts agora").clicked() {
                app.usage_history.refresh();
            }
            if let (Some(r), Some(at)) = (&app.usage_history.report, app.usage_history.received) {
                let s = at.elapsed().as_secs();
                let when = if s < 2 { "agora".to_string() } else { format!("há {s}s") };
                ui.label(widgets::subtle(format!("{} transcripts · lido {when}", r.scan.files), &theme).small());
            }
        });
    });
    ui.label(
        widgets::subtle("Somado dos transcripts do Claude Code (~/.claude/projects), últimos 30 dias. Custo estimado pela tabela de preços da API.", &theme)
            .small(),
    );
    ui.add_space(12.0);

    let Some(report) = app.usage_history.report.clone() else {
        ui.horizontal(|ui| {
            ui.spinner();
            ui.label(widgets::subtle("Lendo os transcripts…", &theme));
        });
        return;
    };
    if report.rows.is_empty() {
        widgets::empty_state(ui, "Nenhum uso nos últimos 30 dias", "Os números aparecem aqui assim que uma sessão do Claude Code responder.", &theme);
        return;
    }

    let palette = Palette::new(&report, &theme);
    stat_cards(app, ui, &report, &theme);
    ui.add_space(14.0);
    widgets::card(ui, &theme, false, theme.border, |ui| {
        ui.set_width(ui.available_width());
        chart(ui, &report, &palette, &theme);
    });
    ui.add_space(14.0);

    let period = app.usage_history.period;
    let label = PERIODS.iter().find(|p| p.0 == period).map(|p| p.1).unwrap_or("");
    ui.columns(2, |cols| {
        widgets::card(&mut cols[0], &theme, false, theme.border, |ui| {
            ui.set_width(ui.available_width());
            section(ui, &format!("TOP SESSÕES · {}", label.to_uppercase()), &theme);
            ui.add_space(6.0);
            top_sessions(ui, app, &report, &theme);
        });
        widgets::card(&mut cols[1], &theme, false, theme.border, |ui| {
            ui.set_width(ui.available_width());
            section(ui, &format!("POR MODELO · {}", label.to_uppercase()), &theme);
            ui.add_space(6.0);
            by_model(ui, &report, period, &palette, &theme);
            ui.add_space(12.0);
            section(ui, &format!("POR PROJETO · {}", label.to_uppercase()), &theme);
            ui.add_space(6.0);
            by_project(ui, &report, period, &theme);
        });
    });

    ui.add_space(10.0);
    if !report.unpriced.is_empty() {
        let list: Vec<String> = report.unpriced.iter().map(|(m, t)| format!("{m} ({} tokens)", fmt_tokens(*t))).collect();
        ui.label(RichText::new(format!("Sem preço conhecido (fora do custo): {}. Configure em ~/.config/aic/prices.toml.", list.join(", "))).color(theme.warn).small());
    }
    ui.label(
        widgets::subtle(
            "Preços: platform.claude.com/docs/en/about-claude/pricing (entrada, saída, escrita de cache 5 min/1 h, leitura de cache; modo rápido 2×). Na assinatura Pro/Max o que conta são os limites acima.",
            &theme,
        )
        .small(),
    );
    ui.add_space(12.0);
}

/// Hoje / 7 dias / 30 dias. Clicar escolhe o período das listas.
fn stat_cards(app: &mut App, ui: &mut Ui, report: &Report, theme: &Theme) {
    let mut pick = None;
    let current = app.usage_history.period;
    ui.columns(3, |cols| {
        for ((days, label), ui) in PERIODS.into_iter().zip(cols.iter_mut()) {
            let t = report.totals(days);
            let active = current == days;
            let resp = widgets::card(ui, theme, active, if active { theme.accent } else { theme.faint }, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    section(ui, &label.to_uppercase(), theme);
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let since = report.since(days);
                        let range = if days == 1 { format!("{} {}", weekday(since), since.format("%d/%m")) } else { format!("{} – {}", since.format("%d/%m"), report.today.format("%d/%m")) };
                        ui.label(RichText::new(range).font(FontId::monospace(11.0)).color(theme.faint));
                    });
                });
                ui.label(RichText::new(fmt_usd(t.cost)).size(30.0).strong().color(theme.ink));
                ui.label(widgets::subtle(format!("{} tokens · {} respostas", fmt_tokens(t.tokens.total()), t.messages), theme));
                ui.add_space(2.0);
                token_split(ui, &t.tokens, theme);
            });
            if resp.on_hover_text("Ver as listas deste período").clicked() {
                pick = Some(days);
            }
        }
    });
    if let Some(d) = pick {
        app.usage_history.period = d;
    }
}

/// `entrada 12K · saída 340K · cache escrito 2.1M · lido 45M`, com os números em destaque.
fn token_split(ui: &mut Ui, t: &Tokens, theme: &Theme) {
    let mut job = egui::text::LayoutJob::default();
    for (i, (k, v)) in [("entrada", t.input), ("saída", t.output), ("cache escrito", t.cache_write), ("lido", t.cache_read)].into_iter().enumerate() {
        if i > 0 {
            widgets::push(&mut job, if i == 3 { " · " } else { "  " }, theme.faint, 11.0);
        }
        widgets::push(&mut job, &format!("{k} "), theme.faint, 11.0);
        widgets::push(&mut job, &fmt_tokens(v), theme.dim, 11.0);
    }
    ui.label(job);
}

/// Barras por dia, empilhadas por modelo (custo). Passe o mouse pra ver o dia.
fn chart(ui: &mut Ui, report: &Report, palette: &Palette, theme: &Theme) {
    let days = report.daily(report.window_days);
    ui.horizontal(|ui| {
        section(ui, "CUSTO POR DIA · POR MODELO", theme);
        ui.add_space(12.0);
        // legenda: sempre presente com 2+ séries; texto em tinta, cor só no quadrado
        let mut shown: Vec<&String> = palette.order.iter().take(MAX_SERIES).collect();
        shown.sort_by_key(|m| palette.slot(m));
        for m in shown {
            legend_key(ui, palette.color(m), &usage::model_label(m), theme);
        }
        if palette.order.len() > MAX_SERIES {
            legend_key(ui, palette.other, "Outros", theme);
        }
    });
    ui.add_space(8.0);

    let height = 190.0;
    let axis_w = 54.0;
    let label_h = 18.0;
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), height + label_h), Sense::hover());
    let plot = Rect::from_min_max(pos2(rect.min.x + axis_w, rect.min.y + 20.0), pos2(rect.max.x - 4.0, rect.min.y + height));
    let p = ui.painter_at(rect);
    let max = days.iter().map(|d| d.cost()).fold(0.0, f64::max);
    let (step, bands) = nice_axis(max);
    let top = step * bands as f64;
    let y_of = |v: f64| plot.max.y - (v / top) as f32 * plot.height();

    // grade recessiva: linhas finas sólidas em valores redondos
    for i in 0..=bands {
        let v = step * i as f64;
        let y = y_of(v).round() + 0.5;
        let color = if i == 0 { theme.border } else { theme.border.gamma_multiply(0.55) };
        p.hline(plot.x_range(), y, Stroke::new(1.0, color));
        let txt = if step < 0.1 { format!("${v:.2}") } else if step < 1.0 { format!("${v:.1}") } else { format!("${v:.0}") };
        p.text(pos2(plot.min.x - 8.0, y), Align2::RIGHT_CENTER, txt, widgets::mono(10.5), theme.faint);
    }

    let n = days.len().max(1);
    let slot = plot.width() / n as f32;
    let bar_w = (slot - 2.0).clamp(2.0, 24.0);
    let hovered = resp.hover_pos().filter(|h| plot.x_range().contains(h.x)).map(|h| (((h.x - plot.min.x) / slot) as usize).min(n - 1));

    if let Some(i) = hovered {
        let x0 = plot.min.x + i as f32 * slot;
        p.rect_filled(Rect::from_min_max(pos2(x0, plot.min.y - 6.0), pos2(x0 + slot, plot.max.y)), CornerRadius::same(theme.radius.min(4)), theme.bg3);
    }

    let gap = 2.0;
    let mut max_at: Option<(usize, f64)> = None;
    for (i, d) in days.iter().enumerate() {
        let cx = plot.min.x + (i as f32 + 0.5) * slot;
        let mut segs: Vec<(usize, Color32, f64)> = Vec::new();
        for m in &d.models {
            let s = palette.slot(&m.key);
            match segs.iter_mut().find(|x| x.0 == s) {
                Some(x) => x.2 += m.cost,
                None => segs.push((s, palette.color(&m.key), m.cost)),
            }
        }
        segs.sort_by_key(|s| s.0);
        segs.retain(|s| s.2 > 0.0);
        let total: f64 = segs.iter().map(|s| s.2).sum();
        if total <= 0.0 {
            continue;
        }
        if max_at.is_none_or(|(_, v)| total > v) {
            max_at = Some((i, total));
        }
        let mut base = plot.max.y;
        let last = segs.len() - 1;
        for (k, (_, color, v)) in segs.iter().enumerate() {
            let h = ((*v / top) as f32 * plot.height()).max(1.0);
            let top_y = base - h;
            // 2px de respiro entre segmentos (cor da superfície), sem contorno
            let bottom = if k > 0 && h > gap + 1.0 { base - gap } else { base };
            let r = Rect::from_min_max(pos2(cx - bar_w / 2.0, top_y), pos2(cx + bar_w / 2.0, bottom));
            let radius = if k == last { (4.0f32).min(h / 2.0) as u8 } else { 0 };
            let cr = CornerRadius { nw: radius, ne: radius, sw: 0, se: 0 };
            let dim = hovered.is_some_and(|hi| hi != i);
            p.rect_filled(r, cr, if dim { color.gamma_multiply(0.55) } else { *color });
            base = top_y;
        }
    }
    // rótulo direto só no pico
    if let Some((i, v)) = max_at {
        let cx = plot.min.x + (i as f32 + 0.5) * slot;
        p.text(pos2(cx, y_of(v) - 4.0), Align2::CENTER_BOTTOM, fmt_usd(v), widgets::mono(10.5), theme.dim);
    }

    // eixo x: a cada 7 dias, terminando em "hoje"
    for (i, d) in days.iter().enumerate() {
        let back = n - 1 - i;
        if !back.is_multiple_of(7) {
            continue;
        }
        let cx = plot.min.x + (i as f32 + 0.5) * slot;
        let txt = if back == 0 { "hoje".to_string() } else { d.day.format("%d/%m").to_string() };
        p.text(pos2(cx, plot.max.y + 5.0), Align2::CENTER_TOP, txt, widgets::mono(10.5), if back == 0 { theme.dim } else { theme.faint });
    }

    if let Some(i) = hovered {
        let d = &days[i];
        resp.on_hover_ui_at_pointer(|ui| {
            ui.label(RichText::new(format!("{} {}", weekday(d.day), d.day.format("%d/%m"))).strong());
            if d.models.is_empty() {
                ui.label(widgets::subtle("sem uso", theme));
                return;
            }
            let mut ms = d.models.clone();
            ms.sort_by(|a, b| b.cost.total_cmp(&a.cost));
            egui::Grid::new("usage-day-tip").num_columns(3).spacing([10.0, 2.0]).show(ui, |ui| {
                for m in &ms {
                    legend_key(ui, palette.color(&m.key), &usage::model_label(&m.key), theme);
                    ui.label(RichText::new(fmt_usd(m.cost)).monospace());
                    ui.label(widgets::subtle(format!("{} tok", fmt_tokens(m.tokens.total())), theme).small());
                    ui.end_row();
                }
                ui.label(RichText::new("Total").strong());
                ui.label(RichText::new(fmt_usd(d.cost())).monospace().strong());
                ui.label(widgets::subtle(format!("{} tok", fmt_tokens(d.tokens().total())), theme).small());
                ui.end_row();
            });
            token_split(ui, &d.tokens(), theme);
        });
    }
}

fn legend_key(ui: &mut Ui, color: Color32, label: &str, theme: &Theme) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 5.0;
        let (r, _) = ui.allocate_exact_size(vec2(10.0, 10.0), Sense::hover());
        ui.painter().rect_filled(r, CornerRadius::same(2), color);
        ui.label(RichText::new(label).color(theme.ink).size(12.0));
    });
}

/// Barra horizontal fina (trilho + preenchimento) proporcional a `frac`.
fn meter(ui: &Ui, rect: Rect, frac: f32, color: Color32, theme: &Theme) {
    let r = rect.height() / 2.0;
    ui.painter().rect_filled(rect, CornerRadius::same(r as u8), theme.bg3);
    let w = (rect.width() * frac.clamp(0.0, 1.0)).max(if frac > 0.0 { rect.height() } else { 0.0 });
    ui.painter().rect_filled(Rect::from_min_size(rect.min, vec2(w, rect.height())), CornerRadius::same(r as u8), color);
}

struct Known {
    id: Id,
    name: String,
    ws: String,
    color: Color32,
}

/// Top sessões do período; as abertas no AIC mostram nome e workspace e são clicáveis.
fn top_sessions(ui: &mut Ui, app: &mut App, report: &Report, theme: &Theme) {
    let mut known: HashMap<String, Known> = HashMap::new();
    for w in &app.workspaces {
        for id in &w.sessions {
            if let Some(rt) = app.sessions.get(id) {
                known.insert(rt.s.claude_session.to_string(), Known { id: *id, name: rt.s.name.clone(), ws: w.name.clone(), color: theme.agent(rt.s.color) });
            }
        }
    }
    let list = report.by_session(app.usage_history.period);
    if list.is_empty() {
        ui.label(widgets::subtle("Sem uso no período.", theme));
        return;
    }
    let max = list.first().map(|b| b.cost).unwrap_or(0.0).max(1e-9);
    let mut clicked = None;
    for b in list.iter().take(8) {
        if let Some(id) = session_row(ui, b, known.get(&b.key), max, theme) {
            clicked = Some(id);
        }
    }
    if list.len() > 8 {
        let rest: f64 = list[8..].iter().map(|b| b.cost).sum();
        ui.label(widgets::subtle(format!("+ {} sessões ({})", list.len() - 8, fmt_usd(rest)), theme).small());
    }
    if let Some(id) = clicked {
        app.focus(id);
    }
}

fn project_name(p: &str) -> String {
    std::path::Path::new(p).file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| p.to_string())
}

fn session_row(ui: &mut Ui, b: &Bucket, known: Option<&Known>, max: f64, theme: &Theme) -> Option<Id> {
    let w = ui.available_width();
    let (rect, resp) = ui.allocate_exact_size(vec2(w, 40.0), if known.is_some() { Sense::click() } else { Sense::hover() });
    let p = ui.painter_at(rect);
    if resp.hovered() {
        p.rect_filled(rect, CornerRadius::same(theme.radius.min(4)), theme.bg3);
    }
    let strip = known.map(|k| k.color).unwrap_or(theme.border);
    p.rect_filled(Rect::from_min_size(rect.min + vec2(0.0, 6.0), vec2(3.0, 28.0)), CornerRadius::same(1), strip);

    let x = rect.min.x + 12.0;
    let right_w = 150.0;
    let (title, sub) = match known {
        Some(k) => (k.name.clone(), format!("{} · {}", k.ws, project_name(&b.project))),
        None => (project_name(&b.project), format!("sessão {}", b.key.get(..8).unwrap_or(&b.key))),
    };
    let title_g = p.layout_no_wrap(title, FontId::proportional(13.5), theme.ink);
    p.galley(pos2(x, rect.min.y + 4.0), title_g.clone(), theme.ink);
    if known.is_some() {
        // etiqueta "aberta" ao lado do nome
        let g = p.layout_no_wrap("no AIC".into(), FontId::proportional(10.5), theme.accent_hi);
        let r = Rect::from_min_size(pos2(x + title_g.size().x + 8.0, rect.min.y + 5.0), g.size() + vec2(10.0, 2.0));
        p.rect(r, CornerRadius::same(if theme.radius == 0 { 0 } else { 99 }), theme.accent.gamma_multiply(0.14), Stroke::new(1.0, theme.accent.gamma_multiply(0.45)), egui::StrokeKind::Inside);
        p.galley(r.min + vec2(5.0, 1.0), g, theme.accent_hi);
    }
    p.text(pos2(x, rect.min.y + 23.0), Align2::LEFT_TOP, format!("{sub} · {}", ago(b.last_ts_ms)), FontId::proportional(11.5), theme.dim);

    let rx = rect.max.x - right_w;
    p.text(pos2(rect.max.x - 4.0, rect.min.y + 4.0), Align2::RIGHT_TOP, fmt_usd(b.cost), widgets::mono(13.0), theme.ink);
    p.text(pos2(rect.max.x - 4.0, rect.min.y + 23.0), Align2::RIGHT_TOP, format!("{} tok", fmt_tokens(b.tokens.total())), widgets::mono(10.5), theme.faint);
    let bar = Rect::from_min_size(pos2(rx - 6.0 - 90.0, rect.center().y - 3.0), vec2(90.0, 6.0));
    meter(ui, bar, (b.cost / max) as f32, theme.accent, theme);

    let resp = match known {
        Some(_) => resp.on_hover_text(format!("Abrir a sessão\n{}", b.project)),
        None => resp.on_hover_text(format!("{}\n{}", b.project, b.key)),
    };
    (resp.clicked()).then(|| known.map(|k| k.id)).flatten()
}

fn by_model(ui: &mut Ui, report: &Report, period: i64, palette: &Palette, theme: &Theme) {
    let list = report.by_model(period);
    if list.is_empty() {
        ui.label(widgets::subtle("Sem uso no período.", theme));
        return;
    }
    let total: f64 = list.iter().map(|b| b.cost).sum::<f64>().max(1e-9);
    let max = list.first().map(|b| b.cost).unwrap_or(0.0).max(1e-9);
    for b in &list {
        let w = ui.available_width();
        let (rect, resp) = ui.allocate_exact_size(vec2(w, 34.0), Sense::hover());
        let p = ui.painter_at(rect);
        let color = palette.color(&b.key);
        p.rect_filled(Rect::from_min_size(rect.min + vec2(0.0, 6.0), vec2(10.0, 10.0)), CornerRadius::same(2), color);
        p.text(pos2(rect.min.x + 18.0, rect.min.y + 4.0), Align2::LEFT_TOP, palette.label(&b.key), FontId::proportional(13.5), theme.ink);
        let t = &b.tokens;
        p.text(
            pos2(rect.min.x + 18.0, rect.min.y + 21.0),
            Align2::LEFT_TOP,
            format!("entrada {} · saída {} · cache {}", fmt_tokens(t.input), fmt_tokens(t.output), fmt_tokens(t.cache_read + t.cache_write)),
            FontId::proportional(11.0),
            theme.dim,
        );
        p.text(pos2(rect.max.x - 4.0, rect.min.y + 4.0), Align2::RIGHT_TOP, fmt_usd(b.cost), widgets::mono(13.0), theme.ink);
        p.text(pos2(rect.max.x - 4.0, rect.min.y + 21.0), Align2::RIGHT_TOP, format!("{:.0}%", b.cost / total * 100.0), widgets::mono(10.5), theme.faint);
        let bar = Rect::from_min_size(pos2(rect.max.x - 90.0 - 96.0, rect.min.y + 9.0), vec2(90.0, 6.0));
        meter(ui, bar, (b.cost / max) as f32, color, theme);
        resp.on_hover_text(format!("{}\n{} respostas", b.key, b.messages));
    }
}

fn by_project(ui: &mut Ui, report: &Report, period: i64, theme: &Theme) {
    let list = report.by_project(period);
    let max = list.first().map(|b| b.cost).unwrap_or(0.0).max(1e-9);
    for b in list.iter().take(5) {
        let w = ui.available_width();
        let (rect, resp) = ui.allocate_exact_size(vec2(w, 22.0), Sense::hover());
        let p = ui.painter_at(rect);
        p.text(pos2(rect.min.x, rect.center().y), Align2::LEFT_CENTER, format!("{}  {}", widgets::I_DIR, project_name(&b.key)), FontId::proportional(13.0), theme.ink);
        p.text(pos2(rect.max.x - 4.0, rect.center().y), Align2::RIGHT_CENTER, fmt_usd(b.cost), widgets::mono(12.5), theme.ink);
        let bar = Rect::from_min_size(pos2(rect.max.x - 90.0 - 96.0, rect.center().y - 3.0), vec2(90.0, 6.0));
        meter(ui, bar, (b.cost / max) as f32, theme.dim, theme);
        resp.on_hover_text(paths::pretty(std::path::Path::new(&b.key)));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formatos() {
        assert_eq!(fmt_tokens(999), "999");
        assert_eq!(fmt_tokens(12_345), "12.3K");
        assert_eq!(fmt_tokens(345_000), "345K");
        assert_eq!(fmt_tokens(4_560_000), "4.6M");
        assert_eq!(fmt_usd(0.0), "$0");
        assert_eq!(fmt_usd(0.004), "<$0.01");
        assert_eq!(fmt_usd(12.345), "$12.35");
        assert_eq!(nice_axis(29.06), (10.0, 3));
        assert_eq!(nice_axis(41.0), (20.0, 3));
        assert_eq!(nice_axis(0.42), (0.2, 3));
    }
}
