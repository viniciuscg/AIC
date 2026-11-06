//! Aba Tarefas, no jeito do "Próximos" do Todoist: data + tarefa, agrupadas por dia. Não tem
//! ligação nenhuma com sessões ou terminais.
//!
//! Tudo tem tecla (fora dos campos): `n` escreve, `h/l` escolhe o dia da nova, `j/k` anda, `J/K` sobe/desce
//! no dia, `x` conclui, `e` edita, `< >` muda o dia, `1 2 3 0` hoje/amanhã/semana/sem data, `H` atrasadas pra
//! hoje, `d` apaga, `u` desfaz, `o` abre o link, `c` alterna Em andamento / Concluídas, `t` tags, `b` bloqueia, `m` motivo
//! ou descrição, `Esc` volta pros terminais. `#tag` no texto da tarefa nova também vira tag.

use aic_core::quickadd::{self, due_label, parse_date};
use aic_core::{Id, TAG_BLOCKED, Task, parse_tags};
use chrono::{Datelike, Duration, Local, NaiveDate};
use egui::{Align, Color32, FontId, Key, Layout, Modifiers, RichText, Sense, Stroke, TextEdit, Ui};

use crate::app::{App, ToastKind, View};
use crate::theme::Theme;

/// Dias sempre mostrados (hoje + 6), mesmo vazios, como o Todoist.
const DAYS_SHOWN: i64 = 7;
/// Dias na régua do topo.
const STRIP_DAYS: i64 = 14;
const MAX_WIDTH: f32 = 900.0;

pub struct TasksState {
    pub input: String,
    /// Data da tarefa nova, como digitada (`hoje`, `sex`, `15/11`).
    pub date: String,
    pub selected: Option<Id>,
    pub focus_input: bool,
    pub editing: Option<(Id, Field, String)>,
    /// Em andamento ou Concluídas (a segunda linha da barra).
    pub tab: Tab,
    scroll_to: Option<NaiveDate>,
    undo: Option<Task>,
}

impl Default for TasksState {
    fn default() -> Self {
        Self {
            input: String::new(),
            date: "hoje".into(),
            selected: None,
            focus_input: false,
            editing: None,
            tab: Tab::Open,
            scroll_to: None,
            undo: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Tab {
    #[default]
    Open,
    Done,
}

impl Tab {
    pub fn label(self) -> &'static str {
        match self {
            Tab::Open => "Em andamento",
            Tab::Done => "Concluídas",
        }
    }
}

/// Troca a subaba (o `c`).
pub fn set_tab(st: &mut TasksState, tab: Tab) {
    if st.tab != tab {
        st.tab = tab;
        st.selected = None;
        st.editing = None;
    }
}

/// Concluídas agrupadas pelo dia (local) em que foram concluídas, a mais recente primeiro.
pub fn done_groups(tasks: &[Task]) -> Vec<(NaiveDate, Vec<&Task>)> {
    let mut done: Vec<&Task> = tasks.iter().filter(|t| t.done).collect();
    done.sort_by_key(|t| std::cmp::Reverse(t.finished_at()));
    let mut out: Vec<(NaiveDate, Vec<&Task>)> = Vec::new();
    for t in done {
        let Some(at) = t.finished_at() else { continue };
        let day = at.with_timezone(&Local).date_naive();
        match out.last_mut() {
            Some((d, v)) if *d == day => v.push(t),
            _ => out.push((day, vec![t])),
        }
    }
    out
}

/// Quantas concluídas desde `since` (dia local, inclusive).
pub fn done_since(tasks: &[Task], since: NaiveDate) -> usize {
    tasks
        .iter()
        .filter_map(|t| t.finished_at())
        .filter(|at| at.with_timezone(&Local).date_naive() >= since)
        .count()
}

/// O que o campo de edição da linha está mexendo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Field {
    Title,
    Tags,
    Notes,
}

impl Field {
    fn prefix(self) -> &'static str {
        match self {
            Field::Title => "",
            Field::Tags => "tags",
            Field::Notes => "motivo / descrição",
        }
    }

    fn hint(self) -> &'static str {
        match self {
            Field::Title => "",
            Field::Tags => "#infra #bloqueado  (Enter salva · Esc cancela)",
            Field::Notes => "por que está bloqueada, ou o que é  (Enter salva · Esc cancela)",
        }
    }

    fn initial(self, t: &Task) -> String {
        match self {
            Field::Title => t.title.clone(),
            Field::Tags => t.tags.iter().map(|g| format!("#{g} ")).collect(),
            Field::Notes => t.notes.trim().replace('\n', " "),
        }
    }
}

fn start_edit(st: &mut TasksState, t: &Task, f: Field) {
    st.editing = Some((t.id, f, f.initial(t)));
}

#[derive(Debug, Clone, PartialEq)]
pub enum Group {
    Overdue,
    Day(NaiveDate),
    Later,
    NoDate,
    /// Na subaba Concluídas: mostra a hora em que concluiu.
    Finished,
}

/// Grupos e suas tarefas abertas, na ordem da tela.
pub fn groups(tasks: &[Task], today: NaiveDate) -> Vec<(Group, Vec<&Task>)> {
    let mut open: Vec<&Task> = tasks.iter().filter(|t| !t.done).collect();
    open.sort_by(|a, b| {
        (a.due, a.rank, a.created_at)
            .partial_cmp(&(b.due, b.rank, b.created_at))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let mut out: Vec<(Group, Vec<&Task>)> = Vec::with_capacity(DAYS_SHOWN as usize + 3);
    let overdue: Vec<&Task> = open
        .iter()
        .copied()
        .filter(|t| t.due.is_some_and(|d| d < today))
        .collect();
    if !overdue.is_empty() {
        out.push((Group::Overdue, overdue));
    }
    for i in 0..DAYS_SHOWN {
        let day = today + Duration::days(i);
        out.push((
            Group::Day(day),
            open.iter()
                .copied()
                .filter(|t| t.due == Some(day))
                .collect(),
        ));
    }
    let horizon = today + Duration::days(DAYS_SHOWN);
    let later: Vec<&Task> = open
        .iter()
        .copied()
        .filter(|t| t.due.is_some_and(|d| d >= horizon))
        .collect();
    if !later.is_empty() {
        out.push((Group::Later, later));
    }
    let nodate: Vec<&Task> = open.iter().copied().filter(|t| t.due.is_none()).collect();
    if !nodate.is_empty() {
        out.push((Group::NoDate, nodate));
    }
    out
}

/// Quantas abertas pra hoje ou atrasadas (o número na barra).
pub fn due_now(tasks: &[Task], today: NaiveDate) -> usize {
    tasks
        .iter()
        .filter(|t| !t.done && t.due.is_some_and(|d| d <= today))
        .count()
}

fn weekday(d: NaiveDate) -> &'static str {
    ["seg", "ter", "qua", "qui", "sex", "sáb", "dom"][d.weekday().num_days_from_monday() as usize]
}

fn month(d: NaiveDate) -> &'static str {
    [
        "jan", "fev", "mar", "abr", "mai", "jun", "jul", "ago", "set", "out", "nov", "dez",
    ][d.month0() as usize]
}

fn day_title(d: NaiveDate, today: NaiveDate) -> String {
    let base = format!("{} {} {}", weekday(d), d.day(), month(d));
    match (d - today).num_days() {
        0 => format!("Hoje · {base}"),
        1 => format!("Amanhã · {base}"),
        _ => base,
    }
}

fn first_url(s: &str) -> Option<&str> {
    s.split_whitespace()
        .find(|w| w.starts_with("https://") || w.starts_with("http://"))
}

/// Abre o campo de escrever (o `Ctrl+B t`).
pub fn open(app: &mut App) {
    app.set_view(View::Tasks);
    app.tasks_ui.focus_input = true;
}

pub fn show(app: &mut App, ui: &mut Ui) {
    let theme = app.theme.clone();
    let today = Local::now().date_naive();
    let mut st = std::mem::take(&mut app.tasks_ui);

    // Lista achatada, pra andar no teclado.
    let order: Vec<Id> = match st.tab {
        Tab::Open => groups(&app.tasks, today)
            .iter()
            .flat_map(|(_, v)| v.iter().map(|t| t.id))
            .collect(),
        Tab::Done => done_groups(&app.tasks)
            .iter()
            .flat_map(|(_, v)| v.iter().map(|t| t.id))
            .collect(),
    };
    if app.modal.is_none() && !ui.ctx().egui_wants_keyboard_input() && st.editing.is_none() {
        keys(app, ui, &mut st, &order, today);
    }

    // coluna centralizada, ocupando a altura toda
    let area = ui.available_rect_before_wrap();
    let width = MAX_WIDTH.min(area.width());
    let column = egui::Rect::from_center_size(area.center(), egui::vec2(width, area.height()));
    let mut c = ui.new_child(egui::UiBuilder::new().max_rect(column));
    match st.tab {
        Tab::Open => {
            strip(app, &mut c, &mut st, today, &theme);
            c.add_space(8.0);
            composer(app, &mut c, &mut st, today, &theme);
            c.add_space(10.0);
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(&mut c, |ui| list(app, ui, &mut st, today, &theme));
        }
        Tab::Done => {
            done_summary(app, &mut c, today, &theme);
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(&mut c, |ui| done_list(app, ui, &mut st, today, &theme));
        }
    }
    ui.allocate_rect(area, Sense::hover());
    app.tasks_ui = st;
}

fn keys(app: &mut App, ui: &Ui, st: &mut TasksState, order: &[Id], today: NaiveDate) {
    let press = |k: Key| ui.input_mut(|i| crate::actions::consume_exact(i, Modifiers::NONE, k));
    let text = |c: &str| {
        ui.input_mut(|i| {
            let before = i.events.len();
            i.events
                .retain(|e| !matches!(e, egui::Event::Text(t) if t == c));
            i.events.len() != before
        })
    };
    let pos = st.selected.and_then(|s| order.iter().position(|i| *i == s));
    let step = |d: i32| -> Option<Id> {
        if order.is_empty() {
            return None;
        }
        let n = order.len() as i32;
        let i = pos.map_or(if d > 0 { 0 } else { n - 1 }, |p| {
            (p as i32 + d).clamp(0, n - 1)
        });
        Some(order[i as usize])
    };
    if press(Key::J) || press(Key::ArrowDown) {
        st.selected = step(1);
    }
    if press(Key::K) || press(Key::ArrowUp) {
        st.selected = step(-1);
    }
    if press(Key::N) || press(Key::A) {
        set_tab(st, Tab::Open);
        st.focus_input = true;
    }
    if press(Key::C) {
        let next = match st.tab {
            Tab::Open => Tab::Done,
            Tab::Done => Tab::Open,
        };
        set_tab(st, next);
        return;
    }
    // h/l: o dia da tarefa nova, na régua
    let day_step = if press(Key::H) {
        -1
    } else if press(Key::L) {
        1
    } else {
        0
    };
    if day_step != 0 {
        let base = parse_date(st.date.trim(), today).unwrap_or(today);
        let d = (base + Duration::days(day_step)).max(today);
        st.date = d.format("%d/%m/%Y").to_string();
        st.scroll_to = Some(d);
    }
    if text("H") {
        send_overdue_to_today(app, today);
    }
    if press(Key::U)
        && let Some(t) = st.undo.take()
    {
        app.toast(&format!("Voltou: {}", t.title), ToastKind::Ok);
        app.save_task(t);
    }
    if press(Key::Escape) {
        app.set_view(View::Sessions);
        return;
    }
    let Some(id) = st.selected.filter(|s| order.contains(s)) else {
        return;
    };
    let next_after = pos.and_then(|p| order.get(p + 1).or(p.checked_sub(1).map(|q| &order[q])));
    if press(Key::X) || press(Key::Space) {
        // na subaba Concluídas, reabre
        app.update_task(id, |t| t.done = !t.done);
        st.selected = next_after.copied();
    }
    if press(Key::D) || press(Key::Delete) {
        remove(app, st, id);
        st.selected = next_after.copied();
    }
    let task = app.tasks.iter().find(|t| t.id == id).cloned();
    if let Some(t) = &task {
        if press(Key::E) || press(Key::Enter) {
            start_edit(st, t, Field::Title);
        }
        if press(Key::T) {
            start_edit(st, t, Field::Tags);
        }
        if press(Key::M) {
            start_edit(st, t, Field::Notes);
        }
        if press(Key::B) {
            toggle_blocked(app, st, t);
        }
    }
    if press(Key::O)
        && let Some(url) = app
            .tasks
            .iter()
            .find(|t| t.id == id)
            .and_then(|t| first_url(&t.title))
    {
        let _ = open::that_detached(url);
    }
    for (key, due) in [
        (Key::Num1, Some(today)),
        (Key::Num2, Some(today + Duration::days(1))),
        (Key::Num3, Some(today + Duration::days(7))),
        (Key::Num0, None),
    ] {
        if press(key) {
            app.update_task(id, |t| t.due = due);
        }
    }
    if text(">") {
        shift(app, id, 1, today);
    }
    if text("<") {
        shift(app, id, -1, today);
    }
    if text("K") {
        reorder(app, id, Move::Up);
    }
    if text("J") {
        reorder(app, id, Move::Down);
    }
    if press(Key::Home) {
        reorder(app, id, Move::Top);
    }
    if press(Key::End) {
        reorder(app, id, Move::Bottom);
    }
}

/// Para onde a tarefa anda dentro do dia.
#[derive(Clone, Copy)]
enum Move {
    Up,
    Down,
    Top,
    Bottom,
}

/// Nova ordem do dia com `id` movida (entre as abertas com o mesmo `due`).
/// Devolve (id, rank) de todas do dia, ou nada se já está no lugar.
fn reordered(tasks: &[Task], id: Id, mv: Move) -> Vec<(Id, f64)> {
    let Some(due) = tasks.iter().find(|t| t.id == id).map(|t| t.due) else {
        return vec![];
    };
    let mut day: Vec<&Task> = tasks.iter().filter(|t| !t.done && t.due == due).collect();
    day.sort_by(|a, b| {
        (a.rank, a.created_at)
            .partial_cmp(&(b.rank, b.created_at))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let Some(p) = day.iter().position(|t| t.id == id) else {
        return vec![];
    };
    let last = day.len() - 1;
    let q = match mv {
        Move::Up => p.saturating_sub(1),
        Move::Down => (p + 1).min(last),
        Move::Top => 0,
        Move::Bottom => last,
    };
    if q == p {
        return vec![];
    }
    let t = day.remove(p);
    day.insert(q, t);
    day.iter()
        .enumerate()
        .map(|(i, t)| (t.id, i as f64))
        .collect()
}

fn reorder(app: &mut App, id: Id, mv: Move) {
    for (tid, rank) in reordered(&app.tasks, id, mv) {
        if app.tasks.iter().any(|t| t.id == tid && t.rank != rank) {
            app.update_task(tid, |t| t.rank = rank);
        }
    }
}

/// Liga/desliga o `bloqueado`. Ao bloquear sem motivo escrito, já abre o campo do motivo.
fn toggle_blocked(app: &mut App, st: &mut TasksState, t: &Task) {
    let mut on = false;
    app.update_task(t.id, |x| on = x.toggle_tag(TAG_BLOCKED));
    if on && t.notes.trim().is_empty() {
        st.editing = Some((t.id, Field::Notes, String::new()));
    }
}

fn send_overdue_to_today(app: &mut App, today: NaiveDate) {
    let ids: Vec<Id> = app
        .tasks
        .iter()
        .filter(|t| !t.done && t.due.is_some_and(|d| d < today))
        .map(|t| t.id)
        .collect();
    for id in ids {
        app.update_task(id, |t| t.due = Some(today));
    }
}

/// Muda o dia. Sem data conta como hoje.
fn shift(app: &mut App, id: Id, days: i64, today: NaiveDate) {
    app.update_task(id, |t| {
        let base = t.due.map_or(today, |d| d.max(today - Duration::days(1)));
        t.due = Some(base + Duration::days(days));
    });
}

fn remove(app: &mut App, st: &mut TasksState, id: Id) {
    if let Some(t) = app.tasks.iter().find(|t| t.id == id).cloned() {
        app.toast(&format!("Apagada: {} (u desfaz)", t.title), ToastKind::Info);
        st.undo = Some(t);
    }
    app.delete_task(id);
}

/// Régua de duas semanas: clicar escolhe o dia da tarefa nova e rola até ele.
fn strip(app: &App, ui: &mut Ui, st: &mut TasksState, today: NaiveDate, theme: &Theme) {
    let chosen = parse_date(st.date.trim(), today);
    let w = ui.available_width();
    let cell_w = w / STRIP_DAYS as f32;
    let (rect, _) = ui.allocate_exact_size(egui::vec2(w, 44.0), Sense::hover());
    for i in 0..STRIP_DAYS {
        let d = today + Duration::days(i);
        let r = egui::Rect::from_min_size(
            rect.min + egui::vec2(i as f32 * cell_w, 0.0),
            egui::vec2(cell_w - 2.0, rect.height()),
        );
        let resp = ui.interact(r, ui.id().with(("dia", i)), Sense::click());
        let sel = chosen == Some(d);
        let count = app
            .tasks
            .iter()
            .filter(|t| !t.done && t.due == Some(d))
            .count();
        let p = ui.painter();
        if sel {
            p.rect_filled(r, theme.radius(), theme.accent);
        } else if resp.hovered() {
            p.rect_filled(r, theme.radius(), theme.bg3);
        }
        let ink = if sel { theme.bg } else { theme.ink };
        let dim = if sel { theme.bg } else { theme.dim };
        p.text(
            r.center_top() + egui::vec2(0.0, 5.0),
            egui::Align2::CENTER_TOP,
            weekday(d),
            FontId::monospace(10.5),
            dim,
        );
        p.text(
            r.center_top() + egui::vec2(0.0, 18.0),
            egui::Align2::CENTER_TOP,
            d.day().to_string(),
            FontId::monospace(14.0),
            ink,
        );
        if count > 0 {
            p.rect_filled(
                egui::Rect::from_center_size(
                    r.center_bottom() - egui::vec2(0.0, 3.0),
                    egui::vec2((count.min(6) * 4) as f32, 2.0),
                ),
                0.0,
                if sel { theme.bg } else { theme.accent_hi },
            );
        }
        if resp
            .on_hover_text("Escolhe o dia da tarefa nova (h / l)")
            .clicked()
        {
            st.date = d.format("%d/%m/%Y").to_string();
            st.scroll_to = Some(d);
            st.focus_input = true;
        }
    }
}

/// Data + tarefa, Enter cria e continua no campo.
fn composer(app: &mut App, ui: &mut Ui, st: &mut TasksState, today: NaiveDate, theme: &Theme) {
    let mut submit = false;
    ui.horizontal(|ui| {
        let date = ui.add(
            TextEdit::singleline(&mut st.date)
                .id(egui::Id::new("tarefa-data"))
                .hint_text("sem data")
                .desired_width(110.0)
                .margin(egui::vec2(8.0, 7.0)),
        );
        date.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::TextEdit, true, "Data"));
        let date =
            date.on_hover_text("hoje, amanhã, sex, 15/11, +3 · Tab vai pro título · Enter cria");
        let title = ui.add(
            TextEdit::singleline(&mut st.input)
                .id(egui::Id::new("tarefa-titulo"))
                .hint_text("o que precisa ser feito?  #tag  (Enter)")
                .desired_width(f32::INFINITY)
                .margin(egui::vec2(8.0, 7.0)),
        );
        title.widget_info(|| {
            egui::WidgetInfo::labeled(egui::WidgetType::TextEdit, true, "Nova tarefa")
        });
        let title = title.on_hover_text(
            "n escreve · #tag vira tag (#bloqueado fica vermelha) · Enter cria · Esc sai do campo",
        );
        if std::mem::take(&mut st.focus_input) {
            title.request_focus();
        }
        let enter = ui.input(|i| i.key_pressed(Key::Enter));
        if enter && (title.lost_focus() || date.lost_focus()) {
            submit = true;
        }
        if title.has_focus() || title.lost_focus() || date.has_focus() {
            ui.input_mut(|i| {
                if i.key_pressed(Key::Escape) {
                    st.focus_input = false;
                }
            });
        }
    });
    let typed = st.date.trim();
    let resolved = parse_date(typed, today);
    let hint = if typed.is_empty() {
        "sem data · hoje, amanhã, sex, 15/11, +3".to_string()
    } else {
        match resolved {
            Some(d) => day_title(d, today),
            None => "data não entendida: hoje, amanhã, sex, 15/11, +3".to_string(),
        }
    };
    ui.label(
        RichText::new(hint)
            .small()
            .color(if typed.is_empty() || resolved.is_some() {
                theme.faint
            } else {
                theme.warn
            }),
    );
    if !submit {
        return;
    }
    let parsed = quickadd::parse(&st.input, today);
    if parsed.title.trim().is_empty() {
        st.focus_input = true;
        return;
    }
    if !typed.is_empty() && resolved.is_none() && parsed.due.is_none() {
        app.toast("Não entendi a data", ToastKind::Warn);
        return;
    }
    let mut t = Task::new(parsed.title);
    t.due = parsed.due.or(resolved);
    t.tags = parsed.tags;
    st.scroll_to = t.due;
    st.selected = Some(t.id);
    app.save_task(t);
    st.input.clear();
    st.focus_input = true;
}

fn list(app: &mut App, ui: &mut Ui, st: &mut TasksState, today: NaiveDate, theme: &Theme) {
    // o `&app.tasks` não pode ficar emprestado enquanto a linha mexe na tarefa
    let owned: Vec<(Group, Vec<Task>)> = groups(&app.tasks, today)
        .into_iter()
        .map(|(g, v)| (g, v.into_iter().cloned().collect()))
        .collect();
    for (group, tasks) in owned {
        let (title, color) = match &group {
            Group::Overdue => (format!("Atrasadas · {}", tasks.len()), theme.err),
            Group::Day(d) if *d == today => (day_title(*d, today), theme.accent_hi),
            Group::Day(d) => (day_title(*d, today), theme.ink),
            Group::Later => ("Mais tarde".to_string(), theme.dim),
            Group::NoDate | Group::Finished => ("Sem data".to_string(), theme.dim),
        };
        ui.add_space(8.0);
        let header = ui.horizontal(|ui| {
            ui.label(RichText::new(&title).strong().color(color));
            if group == Group::Overdue
                && ui
                    .small_button("mandar tudo pra hoje")
                    .on_hover_text("Reagenda as atrasadas pra hoje (H)")
                    .clicked()
            {
                send_overdue_to_today(app, today);
            }
        });
        if let Group::Day(d) = group
            && st.scroll_to == Some(d)
        {
            header.response.scroll_to_me(Some(Align::TOP));
            st.scroll_to = None;
        }
        let r = header.response.rect;
        ui.painter().hline(
            r.left()..=ui.max_rect().right(),
            r.bottom() + 2.0,
            Stroke::new(1.0, theme.border),
        );
        ui.add_space(4.0);
        for t in &tasks {
            row(app, ui, st, t, &group, today, theme);
        }
    }
    let n = app.tasks.iter().filter(|t| t.done).count();
    if n > 0 {
        ui.add_space(12.0);
        if ui
            .add(
                egui::Button::new(RichText::new(format!("▸ Concluídas · {n}")).color(theme.dim))
                    .frame(false),
            )
            .on_hover_text("Ver as concluídas (c)")
            .clicked()
        {
            set_tab(st, Tab::Done);
        }
    }
    ui.add_space(24.0);
}

/// Contadores no topo da subaba Concluídas.
fn done_summary(app: &App, ui: &mut Ui, today: NaiveDate, theme: &Theme) {
    let week = today - Duration::days(today.weekday().num_days_from_monday() as i64);
    let month = today.with_day(1).unwrap_or(today);
    ui.horizontal(|ui| {
        for (label, n) in [
            ("hoje", done_since(&app.tasks, today)),
            ("nesta semana", done_since(&app.tasks, week)),
            ("neste mês", done_since(&app.tasks, month)),
            ("no total", app.tasks.iter().filter(|t| t.done).count()),
        ] {
            ui.label(RichText::new(n.to_string()).strong().color(theme.accent_hi));
            ui.label(RichText::new(label).color(theme.dim));
            ui.add_space(14.0);
        }
    });
    ui.label(
        RichText::new("x reabre · e edita · d apaga · c volta pras em andamento")
            .small()
            .color(theme.faint),
    );
    ui.add_space(6.0);
}

/// Concluídas por dia de conclusão, a mais recente em cima.
fn done_list(app: &mut App, ui: &mut Ui, st: &mut TasksState, today: NaiveDate, theme: &Theme) {
    let owned: Vec<(NaiveDate, Vec<Task>)> = done_groups(&app.tasks)
        .into_iter()
        .map(|(d, v)| (d, v.into_iter().cloned().collect()))
        .collect();
    if owned.is_empty() {
        ui.add_space(20.0);
        ui.label(RichText::new("Nada concluído ainda.").color(theme.dim));
        return;
    }
    for (day, tasks) in owned {
        let title = match (today - day).num_days() {
            0 => format!("Hoje · {} {} {}", weekday(day), day.day(), month(day)),
            1 => format!("Ontem · {} {} {}", weekday(day), day.day(), month(day)),
            _ => format!("{} {} {}", weekday(day), day.day(), month(day)),
        };
        ui.add_space(8.0);
        let header = ui.label(
            RichText::new(format!("{title} · {}", tasks.len()))
                .strong()
                .color(if day == today {
                    theme.accent_hi
                } else {
                    theme.ink
                }),
        );
        let r = header.rect;
        ui.painter().hline(
            r.left()..=ui.max_rect().right(),
            r.bottom() + 2.0,
            Stroke::new(1.0, theme.border),
        );
        ui.add_space(4.0);
        for t in &tasks {
            row(app, ui, st, t, &Group::Finished, today, theme);
        }
    }
    ui.add_space(24.0);
}

const ROW_H: f32 = 26.0;
const NOTE_H: f32 = 16.0;
const ROW_KEYS: &str = "x conclui · e edita · t tags · b bloqueia · m motivo/descrição · J K sobe/desce no dia · Home End topo/fim do dia · < > muda o dia · 1 2 3 0 hoje/amanhã/semana/sem data · d apaga";

/// Chip desenhado direto no painter, alinhado pela direita em `right`. Devolve a largura.
fn paint_chip(ui: &Ui, right: f32, cy: f32, text: &str, color: Color32, theme: &Theme) -> f32 {
    let g = ui
        .painter()
        .layout_no_wrap(text.to_string(), FontId::proportional(11.0), color);
    let size = g.size() + egui::vec2(12.0, 4.0);
    let r = egui::Rect::from_min_size(egui::pos2(right - size.x, cy - size.y / 2.0), size);
    let radius = if theme.radius == 0 { 0.0 } else { 99.0 };
    ui.painter().rect(
        r,
        radius,
        color.gamma_multiply(0.14),
        Stroke::new(1.0, color.gamma_multiply(0.45)),
        egui::StrokeKind::Inside,
    );
    ui.painter().galley(r.min + egui::vec2(6.0, 2.0), g, color);
    size.x
}

fn row(
    app: &mut App,
    ui: &mut Ui,
    st: &mut TasksState,
    t: &Task,
    group: &Group,
    today: NaiveDate,
    theme: &Theme,
) {
    let selected = st.selected == Some(t.id);
    let blocked = t.blocked() && !t.done;
    let editing = st
        .editing
        .as_ref()
        .filter(|(id, _, _)| *id == t.id)
        .map(|(_, f, _)| *f);
    let notes = t.notes.trim();
    let show_notes = !notes.is_empty() && !t.done && editing != Some(Field::Notes);
    let width = ui.available_width();
    let height = ROW_H + if show_notes { NOTE_H } else { 0.0 };
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(width, height), Sense::click());
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, &t.title));
    let line = egui::Rect::from_min_size(rect.min, egui::vec2(width, ROW_H));
    if selected {
        ui.painter().rect_filled(rect, theme.radius(), theme.bg3);
        resp.scroll_to_me(None);
    } else if blocked {
        ui.painter()
            .rect_filled(rect, theme.radius(), theme.err.gamma_multiply(0.08));
    } else if resp.hovered() {
        ui.painter().rect_filled(rect, theme.radius(), theme.bg2);
    }
    if blocked {
        // faixa vermelha à esquerda
        ui.painter().rect_filled(
            egui::Rect::from_min_size(rect.min, egui::vec2(3.0, rect.height())),
            0.0,
            theme.err,
        );
    }

    // o círculo de concluir
    let center = line.left_center() + egui::vec2(11.0, 0.0);
    let hit = egui::Rect::from_center_size(center, egui::vec2(22.0, 26.0));
    let check = ui.interact(hit, ui.id().with(("check", t.id)), Sense::click());
    let ring = if check.hovered() {
        theme.accent_hi
    } else if blocked {
        theme.err
    } else {
        theme.dim
    };
    ui.painter()
        .circle_stroke(center, 6.0, Stroke::new(1.3, ring));
    if t.done {
        ui.painter().circle_filled(center, 4.0, theme.accent);
    }
    let check_tip = if t.done {
        "Reabrir (x)"
    } else {
        "Concluir (x)"
    };
    if check.on_hover_text(check_tip).clicked() {
        let done = !t.done;
        app.update_task(t.id, |x| x.done = done);
    }

    // data à direita, quando o grupo não diz qual é
    let right = line.right() - 8.0;
    let mut title_right = right;
    if *group == Group::Finished
        && let Some(at) = t.finished_at()
    {
        let mut label = at.with_timezone(&Local).format("%H:%M").to_string();
        if let Some(d) = t.due {
            label = format!("era pra {} · {label}", due_label(d, today));
        }
        let g = ui
            .painter()
            .layout_no_wrap(label, FontId::proportional(11.5), theme.dim);
        title_right -= g.size().x + 10.0;
        ui.painter().galley(
            egui::pos2(right - g.size().x, line.center().y - g.size().y / 2.0),
            g,
            theme.dim,
        );
    } else if !matches!(group, Group::Day(_))
        && let Some(d) = t.due
    {
        let label = due_label(d, today);
        let color = if d < today && !t.done {
            theme.err
        } else {
            theme.dim
        };
        let g = ui
            .painter()
            .layout_no_wrap(label, FontId::proportional(11.5), color);
        title_right -= g.size().x + 10.0;
        ui.painter().galley(
            egui::pos2(right - g.size().x, line.center().y - g.size().y / 2.0),
            g,
            color,
        );
    }
    if first_url(&t.title).is_some() {
        title_right -= 18.0;
        let c = egui::pos2(title_right + 8.0, line.center().y);
        let hit = egui::Rect::from_center_size(c, egui::vec2(18.0, 22.0));
        let link = ui.interact(hit, ui.id().with(("link", t.id)), Sense::click());
        ui.painter().text(
            c,
            egui::Align2::CENTER_CENTER,
            super::widgets::I_LINK,
            FontId::monospace(11.0),
            if link.hovered() {
                theme.accent_hi
            } else {
                theme.dim
            },
        );
        if link.on_hover_text("Abrir o link (o)").clicked()
            && let Some(u) = first_url(&t.title)
        {
            let _ = open::that_detached(u);
        }
    }

    // tags, da direita pra esquerda; `bloqueado` vermelho e sempre mais perto do título
    if editing != Some(Field::Tags) {
        let mut tags: Vec<&String> = t.tags.iter().filter(|g| *g != TAG_BLOCKED).collect();
        tags.reverse();
        if t.blocked() {
            tags.push(&t.tags[t.tags.iter().position(|g| g == TAG_BLOCKED).unwrap_or(0)]);
        }
        for g in tags {
            // não deixa as tags comerem o título todo
            if title_right - rect.left() < 160.0 {
                break;
            }
            let color = if g == TAG_BLOCKED && !t.done {
                theme.err
            } else if t.done {
                theme.faint
            } else {
                theme.accent_hi
            };
            title_right -= paint_chip(
                ui,
                title_right - 4.0,
                line.center().y,
                &format!("#{g}"),
                color,
                theme,
            ) + 4.0;
        }
    }

    let text_rect = egui::Rect::from_min_max(
        egui::pos2(line.left() + 26.0, line.top()),
        egui::pos2(title_right - 4.0, line.bottom()),
    );
    if let Some(f) = editing {
        edit(app, ui, st, t.id, f, text_rect, theme);
    } else {
        let color: Color32 = if t.done {
            theme.faint
        } else if blocked {
            theme.err
        } else {
            theme.ink
        };
        let mut text = RichText::new(&t.title).color(color);
        if t.done {
            text = text.strikethrough();
        }
        let mut c = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(text_rect)
                .layout(Layout::left_to_right(Align::Center)),
        );
        c.add(egui::Label::new(text).truncate().selectable(false));
    }

    // motivo/descrição embaixo do título
    if show_notes {
        let r = egui::Rect::from_min_max(
            egui::pos2(line.left() + 26.0, line.bottom() - 4.0),
            egui::pos2(right, rect.bottom()),
        );
        let label = if blocked {
            format!("motivo: {}", notes.replace('\n', " "))
        } else {
            notes.replace('\n', " ")
        };
        let color = if blocked {
            theme.err.gamma_multiply(0.8)
        } else {
            theme.dim
        };
        let mut c = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(r)
                .layout(Layout::left_to_right(Align::Min)),
        );
        c.add(
            egui::Label::new(RichText::new(label).small().color(color))
                .truncate()
                .selectable(false),
        );
    }

    if resp.clicked() {
        st.selected = Some(t.id);
    }
    if resp.double_clicked() {
        start_edit(st, t, Field::Title);
    }
    let tip = if notes.is_empty() {
        format!("{}\n{ROW_KEYS}", t.title)
    } else {
        format!(
            "{}\n\n{}\n\n{ROW_KEYS}",
            t.title,
            super::widgets::truncate(notes, 600)
        )
    };
    let resp = resp.on_hover_text(tip);
    resp.context_menu(|ui| {
        for (label, due) in [
            ("Hoje  (1)", Some(today)),
            ("Amanhã  (2)", Some(today + Duration::days(1))),
            ("Daqui a uma semana  (3)", Some(today + Duration::days(7))),
            ("Sem data  (0)", None),
        ] {
            if ui.button(label).clicked() {
                app.update_task(t.id, |x| x.due = due);
                ui.close();
            }
        }
        ui.separator();
        for (label, mv) in [
            ("Mover pro topo  (Home)", Move::Top),
            ("Subir  (Shift+K)", Move::Up),
            ("Descer  (Shift+J)", Move::Down),
            ("Mover pro fim  (End)", Move::Bottom),
        ] {
            if ui.button(label).clicked() {
                reorder(app, t.id, mv);
                ui.close();
            }
        }
        ui.separator();
        if ui.button("Editar  (e)").clicked() {
            start_edit(st, t, Field::Title);
            ui.close();
        }
        if ui.button("Tags  (t)").clicked() {
            start_edit(st, t, Field::Tags);
            ui.close();
        }
        let block_label = if t.blocked() {
            "Desbloquear  (b)"
        } else {
            "Bloquear  (b)"
        };
        if ui
            .button(RichText::new(block_label).color(theme.err))
            .clicked()
        {
            toggle_blocked(app, st, t);
            ui.close();
        }
        if ui.button("Motivo / descrição  (m)").clicked() {
            start_edit(st, t, Field::Notes);
            ui.close();
        }
        ui.separator();
        if ui
            .button(RichText::new("Apagar  (d)").color(theme.err))
            .clicked()
        {
            remove(app, st, t.id);
            ui.close();
        }
    });
}

fn edit(
    app: &mut App,
    ui: &mut Ui,
    st: &mut TasksState,
    id: Id,
    field: Field,
    rect: egui::Rect,
    theme: &Theme,
) {
    let Some((_, _, text)) = st.editing.as_mut() else {
        return;
    };
    let mut c = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(rect)
            .layout(Layout::left_to_right(Align::Center)),
    );
    if !field.prefix().is_empty() {
        c.label(RichText::new(field.prefix()).small().color(theme.dim));
    }
    let r = c.add(
        TextEdit::singleline(text)
            .id(egui::Id::new(("editar-tarefa", id, field as u8)))
            .hint_text(field.hint())
            .desired_width(f32::INFINITY),
    );
    r.request_focus();
    let (enter, esc) = c.input(|i| (i.key_pressed(Key::Enter), i.key_pressed(Key::Escape)));
    if esc {
        st.editing = None;
    } else if enter {
        let value = text.trim().to_string();
        match field {
            Field::Title if !value.is_empty() => app.update_task(id, |t| t.title = value),
            Field::Title => {}
            Field::Tags => {
                let tags = parse_tags(&value);
                app.update_task(id, |t| t.tags = tags);
            }
            Field::Notes => app.update_task(id, |t| t.notes = value),
        }
        st.editing = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn day(d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 11, d).unwrap()
    }

    fn task(title: &str, due: Option<NaiveDate>) -> Task {
        let mut t = Task::new(title);
        t.due = due;
        t
    }

    #[test]
    fn agrupa_como_o_proximos_do_todoist() {
        let today = day(10);
        let tasks = vec![
            task("atrasada", Some(day(1))),
            task("hoje", Some(day(10))),
            task("amanha", Some(day(11))),
            task("longe", Some(day(28))),
            task("solta", None),
        ];
        let g = groups(&tasks, today);
        assert_eq!(g[0].0, Group::Overdue);
        assert_eq!(g[0].1[0].title, "atrasada");
        assert_eq!(g[1].0, Group::Day(today));
        assert_eq!(g[1].1[0].title, "hoje");
        assert_eq!(g[2].1[0].title, "amanha");
        // dias vazios aparecem, dá pra ver a semana
        assert_eq!(g[3].0, Group::Day(day(12)));
        assert!(g[3].1.is_empty());
        assert_eq!(g[8].0, Group::Later);
        assert_eq!(g[9].0, Group::NoDate);
        assert_eq!(due_now(&tasks, today), 2);
    }

    #[test]
    fn concluida_sai_dos_grupos() {
        let mut t = task("x", Some(day(10)));
        t.done = true;
        let v = vec![t];
        assert!(groups(&v, day(10)).iter().all(|(_, g)| g.is_empty()));
        assert_eq!(due_now(&v, day(10)), 0);
    }

    #[test]
    fn concluida_guarda_a_hora_e_reabrir_esquece() {
        let mut t = task("x", None);
        t.done = true;
        t.touch();
        assert!(t.done_at.is_some());
        let v = vec![t.clone()];
        assert_eq!(done_groups(&v).len(), 1);
        assert_eq!(done_since(&v, Local::now().date_naive()), 1);
        t.done = false;
        t.touch();
        assert_eq!(t.done_at, None);
        assert_eq!(t.finished_at(), None);
    }

    #[test]
    fn j_k_reordena_dentro_do_dia() {
        let today = day(10);
        let mut tasks = vec![
            task("a", Some(today)),
            task("b", Some(today)),
            task("c", Some(today)),
            task("outro dia", Some(day(11))),
        ];
        for (i, t) in tasks.iter_mut().enumerate() {
            t.created_at += Duration::seconds(i as i64);
        }
        let c = tasks[2].id;
        for (id, r) in reordered(&tasks, c, Move::Up) {
            tasks.iter_mut().find(|t| t.id == id).unwrap().rank = r;
        }
        let titles: Vec<&str> = groups(&tasks, today)[0]
            .1
            .iter()
            .map(|t| t.title.as_str())
            .collect();
        assert_eq!(titles, ["a", "c", "b"]);
        // já no topo do dia: não mexe
        let a = tasks[0].id;
        assert!(reordered(&tasks, a, Move::Up).is_empty());
        // b (último) vai pro topo; a e c descem uma casa
        let b = tasks[1].id;
        for (id, r) in reordered(&tasks, b, Move::Top) {
            tasks.iter_mut().find(|t| t.id == id).unwrap().rank = r;
        }
        let titles: Vec<&str> = groups(&tasks, today)[0]
            .1
            .iter()
            .map(|t| t.title.as_str())
            .collect();
        assert_eq!(titles, ["b", "a", "c"]);
        assert!(reordered(&tasks, b, Move::Top).is_empty());
        let a = tasks[0].id;
        for (id, r) in reordered(&tasks, a, Move::Bottom) {
            tasks.iter_mut().find(|t| t.id == id).unwrap().rank = r;
        }
        let titles: Vec<&str> = groups(&tasks, today)[0]
            .1
            .iter()
            .map(|t| t.title.as_str())
            .collect();
        assert_eq!(titles, ["b", "c", "a"]);
    }

    #[test]
    fn tags_e_bloqueado() {
        let mut t = task("x", None);
        assert!(!t.blocked());
        assert!(t.toggle_tag("#Bloqueado"));
        assert!(t.blocked());
        assert!(!t.toggle_tag(TAG_BLOCKED));
        assert!(!t.blocked());
        assert_eq!(parse_tags("#Infra, api  #infra"), vec!["infra", "api"]);
        assert_eq!(
            Field::Tags.initial(&{
                t.tags = parse_tags("a b");
                t.clone()
            }),
            "#a #b "
        );
    }
}
