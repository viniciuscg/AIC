//! A barra: em cima, sempre, **Tarefas · Sessões · Tokens** centralizados (`Ctrl+Tab` anda entre
//! eles). Na aba Sessões, uma segunda linha com as sessões, também centralizada, e à direita
//! dela o terminal em foco: estado, nome, modelo, contexto, branch e custo.
//!
//! - sessão na tela: sublinhada; a que acabou ou precisa de você: colorida; trabalhando: um ponto;
//! - workspaces aparecem só com mais de um, à esquerda da segunda linha;
//! - toda dica diz o atalho.

use aic_core::{Id, SessionKind, SessionState};
use egui::text::{LayoutJob, TextWrapping};
use egui::{Color32, Rect, Response, Sense, Ui, pos2, vec2};

use super::widgets::{self, I_AGENTS, I_BRANCH, I_CTX, push};
use crate::actions::Action;
use crate::app::{App, View};
use crate::theme::Theme;

const ROW1: f32 = 26.0;
const ROW2: f32 = 22.0;
const TAB_FONT: f32 = 13.0;
const FONT: f32 = 12.0;
const GAP: f32 = 14.0;

struct Item {
    label: String,
    job: LayoutJob,
    underline: Option<Color32>,
    dot: Option<Color32>,
}

/// Desenha os itens a partir de `x`; devolve as respostas e onde terminou.
fn paint_items(ui: &mut Ui, row: Rect, x: f32, items: Vec<Item>) -> (Vec<Response>, f32) {
    let mut x = x;
    let mut out = Vec::with_capacity(items.len());
    for it in items {
        let galley = ui.painter().layout_job(it.job);
        let dot_w = if it.dot.is_some() { 8.0 } else { 0.0 };
        let w = galley.size().x + dot_w;
        let rect = Rect::from_min_max(pos2(x - 5.0, row.top()), pos2(x + w + 5.0, row.bottom()));
        let resp = ui.interact(rect, ui.id().with(("barra", &it.label)), Sense::click());
        let label = it.label;
        resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, &label));
        let p = ui.painter();
        if resp.hovered() {
            p.rect_filled(rect, 0.0, ui.visuals().widgets.hovered.weak_bg_fill);
        }
        if let Some(c) = it.dot {
            p.circle_filled(pos2(x + 2.5, row.center().y), 2.5, c);
        }
        let gpos = pos2(x + dot_w, row.center().y - galley.size().y / 2.0);
        p.galley(gpos, galley, Color32::WHITE);
        if let Some(c) = it.underline {
            p.rect_filled(
                Rect::from_min_size(pos2(x - 1.0, row.bottom() - 3.0), vec2(w + 2.0, 2.0)),
                0.0,
                c,
            );
        }
        out.push(resp);
        x += w + GAP;
    }
    (out, x - GAP)
}

fn width_of(ui: &Ui, items: &[Item]) -> f32 {
    let mut w = 0.0;
    for it in items {
        w += ui.painter().layout_job(it.job.clone()).size().x
            + if it.dot.is_some() { 8.0 } else { 0.0 };
    }
    w + GAP * items.len().saturating_sub(1) as f32
}

/// Itens centralizados na linha. Devolve as respostas e até onde foram.
fn centered(ui: &mut Ui, row: Rect, items: Vec<Item>) -> (Vec<Response>, f32) {
    let x = row.center().x - width_of(ui, &items) / 2.0;
    paint_items(ui, row, x.max(row.left() + 4.0), items)
}

pub fn show(app: &mut App, ui: &mut Ui) {
    let theme = app.theme.clone();
    let second = app.view != View::Usage;
    let width = ui.available_width();
    let height = ROW1 + if second { ROW2 } else { 0.0 };
    let (rect, _) = ui.allocate_exact_size(vec2(width, height), Sense::hover());
    let row1 = Rect::from_min_size(rect.min, vec2(width, ROW1));
    tabs(app, ui, row1, &theme);
    prefix_chip(app, ui, row1, &theme);
    if second {
        let row2 = Rect::from_min_size(rect.min + vec2(0.0, ROW1), vec2(width, ROW2));
        if app.view == View::Sessions {
            sessions_row(app, ui, row2, &theme);
        } else if app.view == View::Tasks {
            tasks_row(app, ui, row2, &theme);
        } else {
            other_view_row(app, ui, row2, &theme);
        }
    }
}

/// Tarefas, Sessões, Tokens: nessa ordem, no meio.
fn tabs(app: &mut App, ui: &mut Ui, row: Rect, theme: &Theme) {
    let mut views = vec![];
    if app.cfg.on(aic_core::Module::Tasks) {
        views.push((View::Tasks, Action::ViewTasks));
    }
    views.push((View::Sessions, Action::ViewSessions));
    if app.cfg.on(aic_core::Module::Usage) {
        views.push((View::Usage, Action::ViewUsage));
    }
    let today = chrono::Local::now().date_naive();
    let items: Vec<Item> = views
        .iter()
        .map(|(view, _)| {
            let on = app.view == *view;
            let mut job = LayoutJob::default();
            push(
                &mut job,
                view.label(),
                if on { theme.accent_hi } else { theme.dim },
                TAB_FONT,
            );
            if *view == View::Tasks {
                let n = super::tasks::due_now(&app.tasks, today);
                if n > 0 {
                    push(&mut job, &format!(" {n}"), theme.warn, TAB_FONT - 1.0);
                }
            }
            Item {
                label: format!("Aba {}", view.label()),
                job,
                underline: on.then_some(theme.accent_hi),
                dot: None,
            }
        })
        .collect();
    let (resps, _) = centered(ui, row, items);
    for (resp, (view, action)) in resps.into_iter().zip(views) {
        let keys = app.keymap.keys_of(Action::NextView);
        let tip = app.keymap.tip(action, view.label());
        if resp
            .on_hover_text(format!("{tip}\n{keys} anda entre as abas"))
            .clicked()
        {
            match view {
                View::Tasks => super::tasks::open(app),
                v => {
                    app.set_view(v);
                }
            }
        }
    }
}

fn prefix_chip(app: &App, ui: &mut Ui, row: Rect, theme: &Theme) {
    if !app.prefix_pending {
        return;
    }
    let mut job = LayoutJob::default();
    push(
        &mut job,
        &format!("{} {}", widgets::I_KEY, super::prefix::label(app)),
        theme.bg,
        FONT,
    );
    let g = ui.painter().layout_job(job);
    let r = Rect::from_min_size(row.min + vec2(6.0, 3.0), g.size() + vec2(12.0, 4.0));
    let resp = ui.interact(r, ui.id().with("prefixo"), Sense::hover());
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Label, true, "Prefixo armado"));
    resp.on_hover_text("Prefixo armado: aperte a tecla da ação (Esc cancela, ? mostra todas)");
    ui.painter().rect_filled(r, 2.0, theme.warn);
    ui.painter().galley(r.min + vec2(6.0, 2.0), g, theme.bg);
}

/// O pior estado entre os terminais da sessão (o que mais pede atenção).
fn block_state(app: &App, panes: &[Id]) -> (SessionState, bool) {
    let mut worst = SessionState::Idle;
    let mut unseen = false;
    let rank = |s: SessionState| match s {
        SessionState::Waiting => 5,
        SessionState::Done => 4,
        SessionState::Working => 3,
        SessionState::GameOver => 2,
        SessionState::Boot => 1,
        SessionState::Idle => 0,
    };
    for p in panes {
        if let Some(r) = app.sessions.get(p) {
            let s = r.state();
            if rank(s) > rank(worst) {
                worst = s;
            }
            unseen |= r.unseen;
        }
    }
    (worst, unseen)
}

fn sessions_row(app: &mut App, ui: &mut Ui, row: Rect, theme: &Theme) {
    if app.workspaces.len() > 1 && app.cfg.on(aic_core::Module::Workspaces) {
        workspaces(app, ui, row, theme);
    }
    let current = app.current_block();
    let blocks = app.ws_blocks();
    let mut items: Vec<Item> = Vec::with_capacity(blocks.len() + 1);
    let mut states = Vec::with_capacity(blocks.len());
    for (i, id) in blocks.iter().enumerate() {
        let Some(b) = app.blocks.get(id) else {
            continue;
        };
        let (state, unseen) = block_state(app, &b.panes);
        let on = current == Some(*id);
        let urgent =
            !on && (state == SessionState::Waiting || (unseen && state == SessionState::Done));
        let name_c = if on {
            theme.ink
        } else if urgent {
            theme.state_color(state)
        } else {
            theme.dim
        };
        let mut job = LayoutJob::default();
        if i < 9 {
            push(&mut job, &format!("{} ", i + 1), theme.faint, FONT - 1.0);
        }
        push(&mut job, &widgets::truncate(&b.name, 24), name_c, FONT);
        items.push(Item {
            label: format!("Sessão {} {}", i + 1, b.name),
            job,
            underline: on.then_some(theme.accent),
            dot: (state == SessionState::Working).then_some(theme.ok),
        });
        states.push((*id, b.name.clone(), state, i));
    }
    let mut job = LayoutJob::default();
    push(&mut job, widgets::I_PLUS, theme.faint, FONT - 2.0);
    items.push(Item {
        label: "Nova sessão".into(),
        job,
        underline: None,
        dot: None,
    });
    let (mut resps, end) = centered(ui, row, items);
    let plus = resps.pop();
    for (resp, (id, name, state, i)) in resps.into_iter().zip(states) {
        let goto = if i < 9 {
            format!(" · {}", app.keymap.keys_of(Action::Goto((i + 1) as u8)))
        } else {
            String::new()
        };
        let resp = resp.on_hover_text(format!(
            "{name} · {}{goto}\nClique direito: opções",
            crate::brand::state_label(state),
        ));
        if resp.clicked() {
            app.focus_block(id);
            app.set_view(View::Sessions);
        }
        if resp.double_clicked() {
            app.run_action(ui.ctx(), Action::RenameBlock);
        }
        block_menu(app, &resp, id);
    }
    if let Some(p) = plus
        && p.on_hover_text(app.keymap.tip(Action::NewBlock, "Nova sessão"))
            .clicked()
    {
        app.run_action(ui.ctx(), Action::NewBlock);
    }
    focus_status(app, ui, row, end + 16.0, theme);
}

/// Na aba Tarefas: Em andamento · Concluídas, igual à linha das sessões.
fn tasks_row(app: &mut App, ui: &mut Ui, row: Rect, theme: &Theme) {
    use super::tasks::{Tab, set_tab};
    let today = chrono::Local::now().date_naive();
    let open = app.tasks.iter().filter(|t| !t.done).count();
    let blocked = app.tasks.iter().filter(|t| !t.done && t.blocked()).count();
    let done_today = super::tasks::done_since(&app.tasks, today);
    let tabs = [Tab::Open, Tab::Done];
    let items: Vec<Item> = tabs
        .iter()
        .map(|tab| {
            let on = app.tasks_ui.tab == *tab;
            let mut job = LayoutJob::default();
            push(
                &mut job,
                tab.label(),
                if on { theme.ink } else { theme.dim },
                FONT,
            );
            match tab {
                Tab::Open => {
                    push(&mut job, &format!(" {open}"), theme.faint, FONT - 1.0);
                    if blocked > 0 {
                        push(
                            &mut job,
                            &format!(" · {blocked} bloq."),
                            theme.err,
                            FONT - 1.0,
                        );
                    }
                }
                Tab::Done if done_today > 0 => {
                    push(
                        &mut job,
                        &format!(" +{done_today} hoje"),
                        theme.ok,
                        FONT - 1.0,
                    );
                }
                Tab::Done => {}
            }
            Item {
                label: format!("Tarefas {}", tab.label()),
                job,
                underline: on.then_some(theme.accent),
                dot: None,
            }
        })
        .collect();
    let (resps, _) = centered(ui, row, items);
    for (resp, tab) in resps.into_iter().zip(tabs) {
        if resp
            .on_hover_text(format!("{} · c alterna", tab.label()))
            .clicked()
        {
            set_tab(&mut app.tasks_ui, tab);
        }
    }
}

/// Numa tela fora da barra (Config, Worktrees…): o nome dela e como voltar.
fn other_view_row(app: &mut App, ui: &mut Ui, row: Rect, theme: &Theme) {
    let mut job = LayoutJob::default();
    push(&mut job, app.view.label(), theme.accent_hi, FONT);
    let (resps, _) = centered(
        ui,
        row,
        vec![Item {
            label: format!("Aba {}", app.view.label()),
            job,
            underline: None,
            dot: None,
        }],
    );
    for r in resps {
        r.on_hover_text(
            app.keymap
                .tip(Action::CloseFocused, "Voltar pros terminais"),
        );
    }
}

fn block_menu(app: &mut App, resp: &egui::Response, id: Id) {
    resp.context_menu(|ui| {
        let ctx = ui.ctx().clone();
        for (label, a) in [
            ("Novo Claude aqui", Action::NewSession),
            ("Novo terminal aqui", Action::NewShell),
            ("Renomear", Action::RenameBlock),
            ("Mudar o arranjo", Action::CycleSplit),
            ("Mover pra esquerda", Action::MoveBlockUp),
            ("Mover pra direita", Action::MoveBlockDown),
            ("Fechar a sessão inteira", Action::CloseBlock),
        ] {
            let keys = app.keymap.keys_of(a);
            if ui.button(format!("{label}  ({keys})")).clicked() {
                app.focus_block(id);
                app.set_view(View::Sessions);
                app.run_action(&ctx, a);
                ui.close();
            }
        }
    });
}

/// O terminal em foco, alinhado à direita, no espaço que sobrar depois das sessões.
fn focus_status(app: &App, ui: &mut Ui, row: Rect, from: f32, theme: &Theme) {
    let Some(rt) = app.focused().and_then(|id| app.sessions.get(&id)) else {
        return;
    };
    let room = row.right() - 8.0 - from;
    if room < 120.0 {
        return;
    }
    let state = rt.state();
    let mut job = LayoutJob::default();
    job.wrap = TextWrapping::truncate_at_width(room);
    let dot = |job: &mut LayoutJob| push(job, " · ", theme.faint, FONT - 1.0);
    let f = FONT - 1.0;
    push(
        &mut job,
        crate::brand::state_label(state),
        theme.state_color(state),
        f,
    );
    dot(&mut job);
    push(&mut job, &widgets::truncate(&rt.s.name, 28), theme.dim, f);
    match (&rt.s.kind, &rt.snap) {
        (SessionKind::Claude, Some(s)) => {
            dot(&mut job);
            push(&mut job, &s.model, theme.dim, f);
            if let Some(pct) = s.context_pct {
                dot(&mut job);
                push(
                    &mut job,
                    &format!("{I_CTX} {pct:.0}%"),
                    theme.usage_color(pct),
                    f,
                );
            }
        }
        (SessionKind::Claude, None) => {}
        (SessionKind::Shell, _) => {
            dot(&mut job);
            push(&mut job, "shell", theme.dim, f);
        }
        (SessionKind::Command(c), _) => {
            dot(&mut job);
            push(
                &mut job,
                &format!(
                    "$ {}",
                    widgets::truncate(c.lines().next().unwrap_or(""), 24)
                ),
                theme.dim,
                f,
            );
        }
    }
    if let Some(g) = &rt.git
        && let Some(b) = &g.branch
    {
        dot(&mut job);
        push(
            &mut job,
            &format!("{I_BRANCH} {}", widgets::truncate(b, 32)),
            theme.dim,
            f,
        );
        if g.dirty > 0 {
            push(&mut job, &format!(" ±{}", g.dirty), theme.warn, f);
        }
    }
    if let Some(s) = rt.snap.as_ref().filter(|s| s.cost_usd > 0.0) {
        dot(&mut job);
        push(
            &mut job,
            &crate::usage_history::fmt_usd(s.cost_usd),
            theme.dim,
            f,
        );
    }
    let agents = rt.live.agents.len();
    if agents > 0 {
        dot(&mut job);
        push(
            &mut job,
            &format!("{I_AGENTS} {agents}"),
            theme.attention,
            f,
        );
    }
    let g = ui.painter().layout_job(job);
    let pos = pos2(
        row.right() - 8.0 - g.size().x,
        row.center().y - g.size().y / 2.0,
    );
    let r = Rect::from_min_size(pos, g.size());
    ui.interact(r, ui.id().with("status"), Sense::hover())
        .on_hover_text(format!(
            "{}\n{} anda entre os terminais",
            aic_core::paths::pretty(&rt.s.cwd),
            app.keymap.keys_of(Action::NextSession)
        ));
    ui.painter().galley(pos, g, Color32::WHITE);
}

/// Um item por workspace (só com mais de um), à esquerda da linha das sessões.
fn workspaces(app: &mut App, ui: &mut Ui, row: Rect, theme: &Theme) {
    let names: Vec<(String, usize)> = app
        .workspaces
        .iter()
        .map(|w| (w.name.clone(), w.sessions.len()))
        .collect();
    let items: Vec<Item> = names
        .iter()
        .enumerate()
        .map(|(i, (name, _))| {
            let current = i == app.ws;
            let color = if current {
                theme.accent_hi
            } else if app.ws_needs_you(i) > 0 {
                theme.attention
            } else {
                theme.faint
            };
            let mut job = LayoutJob::default();
            push(&mut job, name, color, FONT - 1.0);
            Item {
                label: format!("Workspace {} {name}", i + 1),
                job,
                underline: None,
                dot: None,
            }
        })
        .collect();
    let (resps, _) = paint_items(ui, row, row.left() + 8.0, items);
    let mut pick = None;
    for (i, resp) in resps.into_iter().enumerate() {
        let (name, n) = &names[i];
        let keys = if i < 9 {
            app.keymap.keys_of(Action::GotoWorkspace((i + 1) as u8))
        } else {
            String::new()
        };
        let resp = resp.on_hover_text(format!("Workspace {name} · {n} terminais · {keys}"));
        if resp.clicked() {
            pick = Some(i);
        }
        resp.context_menu(|ui| {
            let (a, b) = (
                app.keymap.keys_of(Action::RenameWorkspace),
                app.keymap.keys_of(Action::Workspaces),
            );
            if ui.button(format!("Renomear  ({a})")).clicked() {
                app.switch_ws(i);
                app.run_action(ui.ctx(), Action::RenameWorkspace);
                ui.close();
            }
            if ui.button(format!("Gerenciar workspaces…  ({b})")).clicked() {
                app.run_action(ui.ctx(), Action::Workspaces);
                ui.close();
            }
        });
    }
    if let Some(i) = pick {
        app.switch_ws(i);
    }
}
