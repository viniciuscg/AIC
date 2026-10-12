//! Barra de status: a statusline do Claude da sessão em foco, desenhada pelo app.
//!
//! ```text
//!  Opus 5.5 · medium   api-pagamentos   DEV-142   main ±8   󰧑 █░░░░░░░░░ 17% de 1M
//! 󱑂 5h █░░░ 15% reseta em 3h13m   󰃭 semana ░░░ 2% reseta em 6d11h   󰄦 $3.83 · 1065min   ⏵⏵ bypass   ← 4 agents
//! ```

use egui::text::LayoutJob;
use egui::{Sense, Ui};
use aic_core::SessionKind;

use super::widgets::{self, *};
use crate::app::App;

const SZ: f32 = 12.5;

pub fn show(app: &mut App, ui: &mut Ui) {
    let theme = app.theme.clone();
    let Some(id) = app.focused() else {
        let k = |a: crate::actions::Action| app.keymap.label(a);
        ui.label(widgets::subtle(
            format!(
                "Sem sessão em foco · {} abre um Claude · {} cria uma tarefa · {} menu de atalhos",
                k(crate::actions::Action::NewSession),
                k(crate::actions::Action::QuickAdd),
                super::leader::label(app)
            ),
            &theme,
        ));
        return;
    };
    let task = app.task_of(id).cloned();
    let Some(rt) = app.sessions.get(&id) else { return };
    let sep = |job: &mut LayoutJob| push(job, "  ", theme.faint, SZ);

    // ── linha 1 ──
    let mut l1 = LayoutJob::default();
    match (&rt.s.kind, &rt.snap) {
        (SessionKind::Claude, Some(s)) => {
            push(&mut l1, &s.model, theme.ink, SZ);
            if let Some(e) = &s.effort {
                push(&mut l1, &format!(" · {e}"), theme.faint, SZ);
            }
        }
        (SessionKind::Claude, None) => push(&mut l1, "Claude", theme.ink, SZ),
        (SessionKind::Shell, _) => push(&mut l1, "shell", theme.ink, SZ),
        (SessionKind::Command(c), _) => push(&mut l1, &format!("$ {}", truncate(c.lines().next().unwrap_or(""), 40)), theme.ink, SZ),
    }
    sep(&mut l1);
    push(&mut l1, &format!("{I_DIR} {}", rt.display_dir()), theme.dim, SZ);
    if let Some(t) = &task {
        sep(&mut l1);
        let label = t.card_key.clone().unwrap_or_else(|| truncate(&t.title, 28));
        push(&mut l1, &format!("{I_TASK} {label}"), theme.accent_hi, SZ);
    }
    if let Some(g) = &rt.git {
        if let Some(b) = &g.branch {
            sep(&mut l1);
            push(&mut l1, &format!("{I_BRANCH} {b}"), theme.dim, SZ);
            if g.dirty > 0 {
                push(&mut l1, &format!(" ±{}", g.dirty), theme.warn, SZ);
            }
            if g.ahead > 0 {
                push(&mut l1, &format!(" ↑{}", g.ahead), theme.ok, SZ);
            }
            if g.behind > 0 {
                push(&mut l1, &format!(" ↓{}", g.behind), theme.err, SZ);
            }
        }
    }
    if rt.s.worktree.is_some() {
        push(&mut l1, &format!(" {I_WORKTREE}"), theme.accent, SZ);
    }
    if let Some(s) = &rt.snap {
        if let Some(pct) = s.context_pct {
            sep(&mut l1);
            push(&mut l1, &format!("{I_CTX} "), theme.dim, SZ);
            bar_job(&mut l1, pct, 10, &theme, SZ);
            let size = s.context_size.unwrap_or(200_000);
            let label = if size >= 1_000_000 { format!("{}M", size / 1_000_000) } else { format!("{}k", size / 1000) };
            push(&mut l1, &format!(" {pct:.0}% de {label}"), theme.dim, SZ);
        }
    }

    // ── linha 2 ──
    let mut l2 = LayoutJob::default();
    let acc = rt.snap.as_ref().filter(|s| s.five_hour.is_some()).or(app.account.as_ref());
    let mut first = true;
    let mut part = |job: &mut LayoutJob| {
        if !first {
            push(job, "  ", theme.faint, SZ);
        }
        first = false;
    };
    if let Some(a) = acc {
        if let Some(h) = a.five_hour {
            part(&mut l2);
            push(&mut l2, &format!("{I_5H} 5h "), theme.dim, SZ);
            bar_job(&mut l2, h.used_pct, 10, &theme, SZ);
            push(&mut l2, &format!(" {:.0}%", h.used_pct), theme.usage_color(h.used_pct), SZ);
            push(&mut l2, &format!(" reseta em {}", until(h.resets_at)), theme.dim, SZ);
        }
        if let Some(w) = a.seven_day {
            part(&mut l2);
            push(&mut l2, &format!("{I_WEEK} semana "), theme.dim, SZ);
            bar_job(&mut l2, w.used_pct, 10, &theme, SZ);
            push(&mut l2, &format!(" {:.0}%", w.used_pct), theme.usage_color(w.used_pct), SZ);
            push(&mut l2, &format!(" reseta em {}", until(w.resets_at)), theme.dim, SZ);
        }
        if let Some(sp) = a.spend {
            part(&mut l2);
            push(&mut l2, &format!("{I_SPEND} gasto "), theme.dim, SZ);
            bar_job(&mut l2, sp.used_pct, 10, &theme, SZ);
            push(&mut l2, &format!(" {:.0}%", sp.used_pct), theme.usage_color(sp.used_pct), SZ);
        }
    }
    if let Some(s) = &rt.snap {
        part(&mut l2);
        push(&mut l2, &format!("{I_COST} ${:.2} · {}min", s.cost_usd, s.duration_ms / 60_000), theme.dim, SZ);
        if s.lines_added + s.lines_removed > 0 {
            push(&mut l2, &format!("  +{} -{}", s.lines_added, s.lines_removed), theme.faint, SZ);
        }
    }
    if let Some(mode) = rt.live.permission_mode.as_deref().filter(|m| *m != "default") {
        part(&mut l2);
        let (label, color) = match mode {
            "bypassPermissions" => ("⏵⏵ bypass permissions on", theme.err),
            "acceptEdits" => ("⏵⏵ accept edits on", theme.warn),
            "plan" => ("⏸ plan mode on", theme.accent),
            m => (m, theme.dim),
        };
        push(&mut l2, label, color, SZ);
    }
    let agents = rt.live.agents.len();
    if agents > 0 {
        part(&mut l2);
        push(&mut l2, &format!("← {agents} agent{}", if agents == 1 { "" } else { "s" }), theme.attention, SZ);
    }
    if l2.is_empty() {
        push(&mut l2, &format!("{} · {}", crate::brand::state_label(rt.state()), aic_core::paths::pretty(&rt.s.cwd)), theme.faint, SZ);
    }

    ui.add(egui::Label::new(l1).sense(Sense::hover()).wrap_mode(egui::TextWrapMode::Truncate));
    ui.add(egui::Label::new(l2).sense(Sense::hover()).wrap_mode(egui::TextWrapMode::Truncate));
}

pub fn truncate(s: &str, n: usize) -> String {
    if s.chars().count() <= n { s.to_string() } else { format!("{}…", s.chars().take(n - 1).collect::<String>()) }
}
