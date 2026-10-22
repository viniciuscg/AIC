//! Processo no PTY + estado do terminal (alacritty_terminal), sem nada de UI.

use std::borrow::Cow;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use alacritty_terminal::event::{Event, EventListener, Notify, OnResize, WindowSize};
use alacritty_terminal::event_loop::{EventLoop, EventLoopSender, Msg, Notifier};
use alacritty_terminal::grid::{Dimensions, Scroll};
use alacritty_terminal::index::{Column, Direction, Line, Point, Side};
use alacritty_terminal::selection::{Selection, SelectionType};
use alacritty_terminal::sync::FairMutex;
use alacritty_terminal::term::search::{Match, RegexIter, RegexSearch};
use alacritty_terminal::term::{self, Term, TermMode, viewport_to_point};
use alacritty_terminal::tty;

use crate::theme::TerminalTheme;

/// O que rodar no terminal.
#[derive(Debug, Clone)]
pub struct TermSettings {
    pub program: String,
    pub args: Vec<String>,
    pub cwd: Option<PathBuf>,
    pub env: HashMap<String, String>,
    pub scrollback: usize,
}

impl Default for TermSettings {
    fn default() -> Self {
        Self {
            program: std::env::var("SHELL").unwrap_or_else(|_| "/bin/bash".into()),
            args: vec![],
            cwd: None,
            env: HashMap::new(),
            scrollback: 10_000,
        }
    }
}

/// Estado compartilhado entre a thread do PTY e a UI. Tudo atômico para a UI
/// nunca travar esperando o PTY.
#[derive(Default)]
pub struct Shared {
    /// Último output do processo (ms desde epoch). Base da heurística WORKING/IDLE.
    pub last_output_ms: AtomicU64,
    pub bells: AtomicU64,
    pub exited: AtomicBool,
    pub exit_code: AtomicI32,
    pub title: Mutex<Option<String>>,
    pub clipboard: Mutex<Option<String>>,
    theme: Mutex<TerminalTheme>,
    size: Mutex<Option<WindowSize>>,
    writer: Mutex<Option<EventLoopSender>>,
}

pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Intervalo mínimo entre redesenhos pedidos pelos terminais (~30 fps).
const FRAME_MS: u64 = 33;

/// Quadro (ms desde epoch) do último redesenho pedido por QUALQUER terminal: o imediato ou o
/// agendado. Compartilhado: vários Claudes animando o spinner ao mesmo tempo somam um redesenho
/// por quadro, não um por pedaço de saída que chega.
static FRAME_AT_MS: AtomicU64 = AtomicU64::new(0);

/// Pede um redesenho limitado a ~30 fps. Dentro do intervalo, agenda pro próximo quadro: a
/// última saída sempre aparece, só que no máximo um quadro depois.
fn request_repaint_coalesced(ctx: &egui::Context) {
    let now = now_ms();
    let last = FRAME_AT_MS.load(Ordering::Relaxed);
    if now >= last + FRAME_MS {
        // livre: desenha já
        let _ = FRAME_AT_MS.compare_exchange(last, now, Ordering::Relaxed, Ordering::Relaxed);
        ctx.request_repaint();
    } else if last > now {
        // já tem quadro agendado: ele pega esta saída também
        ctx.request_repaint_after(Duration::from_millis(last - now));
    } else {
        let at = last + FRAME_MS;
        let _ = FRAME_AT_MS.compare_exchange(last, at, Ordering::Relaxed, Ordering::Relaxed);
        ctx.request_repaint_after(Duration::from_millis(at - now));
    }
}

#[derive(Clone)]
pub struct EventProxy {
    shared: Arc<Shared>,
    ctx: egui::Context,
}

impl EventProxy {
    fn write(&self, s: String) {
        if let Some(tx) = self.shared.writer.lock().unwrap().as_ref() {
            let _ = tx.send(Msg::Input(Cow::Owned(s.into_bytes())));
        }
    }
}

impl EventListener for EventProxy {
    fn send_event(&self, event: Event) {
        let s = &self.shared;
        match event {
            Event::Wakeup => {
                s.last_output_ms.store(now_ms(), Ordering::Relaxed);
                request_repaint_coalesced(&self.ctx);
            }
            Event::Title(t) => *s.title.lock().unwrap() = Some(t),
            Event::ResetTitle => *s.title.lock().unwrap() = None,
            Event::Bell => {
                s.bells.fetch_add(1, Ordering::Relaxed);
                request_repaint_coalesced(&self.ctx);
            }
            Event::ClipboardStore(_, text) => *s.clipboard.lock().unwrap() = Some(text),
            // Respostas que o programa espera (DA, cor de fundo, tamanho): sem elas
            // TUIs como o Claude Code ficam esperando ou erram o tema.
            Event::PtyWrite(text) => self.write(text),
            Event::ColorRequest(index, fmt) => {
                let rgb = s.theme.lock().unwrap().rgb_of(index);
                self.write(fmt(rgb));
            }
            Event::TextAreaSizeRequest(fmt) => {
                if let Some(size) = *s.size.lock().unwrap() {
                    self.write(fmt(size));
                }
            }
            Event::ChildExit(status) => {
                s.exit_code
                    .store(status.code().unwrap_or(-1), Ordering::Relaxed);
                s.exited.store(true, Ordering::Relaxed);
                self.ctx.request_repaint();
            }
            Event::Exit => {
                s.exited.store(true, Ordering::Relaxed);
                self.ctx.request_repaint();
            }
            _ => {}
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GridSize {
    pub cols: usize,
    pub lines: usize,
    pub cell_w: f32,
    pub cell_h: f32,
}

impl Dimensions for GridSize {
    fn total_lines(&self) -> usize {
        self.lines
    }
    fn screen_lines(&self) -> usize {
        self.lines
    }
    fn columns(&self) -> usize {
        self.cols
    }
}

impl GridSize {
    fn window_size(&self) -> WindowSize {
        WindowSize {
            num_lines: self.lines as u16,
            num_cols: self.cols as u16,
            cell_width: self.cell_w as u16,
            cell_height: self.cell_h as u16,
        }
    }
}

pub struct TerminalBackend {
    pub id: u64,
    pub pid: u32,
    pub shared: Arc<Shared>,
    pub(crate) term: Arc<FairMutex<Term<EventProxy>>>,
    notifier: Notifier,
    pub(crate) size: GridSize,
    url_regex: RegexSearch,
    pub(crate) hovered_link: Option<Match>,
}

impl TerminalBackend {
    pub fn spawn(
        id: u64,
        ctx: egui::Context,
        settings: TermSettings,
        theme: TerminalTheme,
    ) -> anyhow::Result<Self> {
        let size = GridSize {
            cols: 100,
            lines: 30,
            cell_w: 8.0,
            cell_h: 16.0,
        };
        let mut env = settings.env.clone();
        env.entry("TERM".into())
            .or_insert_with(|| "xterm-256color".into());
        env.entry("COLORTERM".into())
            .or_insert_with(|| "truecolor".into());
        env.insert("AIC_TERM_ID".into(), id.to_string());

        let opts = tty::Options {
            shell: Some(tty::Shell::new(settings.program, settings.args)),
            working_directory: settings.cwd,
            drain_on_exit: true,
            env,
        };

        let shared = Arc::new(Shared::default());
        *shared.theme.lock().unwrap() = theme;
        *shared.size.lock().unwrap() = Some(size.window_size());
        shared.last_output_ms.store(now_ms(), Ordering::Relaxed);

        let proxy = EventProxy {
            shared: shared.clone(),
            ctx,
        };
        let config = term::Config {
            scrolling_history: settings.scrollback,
            ..Default::default()
        };
        let term = Arc::new(FairMutex::new(Term::new(config, &size, proxy.clone())));

        let pty = tty::new(&opts, size.window_size(), id)?;
        let pid = pty.child().id();
        let event_loop = EventLoop::new(term.clone(), proxy, pty, true, false)?;
        let tx = event_loop.channel();
        *shared.writer.lock().unwrap() = Some(tx.clone());
        event_loop.spawn();

        Ok(Self {
            id,
            pid,
            shared,
            term,
            notifier: Notifier(tx),
            size,
            url_regex: RegexSearch::new(
                r#"(https?://|file://|git://|ssh:|mailto:)[^\u{0000}-\u{001F}\u{007F}-\u{009F}<>"\s{-}\^⟨⟩`]+"#,
            )
            .expect("regex de URL"),
            hovered_link: None,
        })
    }

    pub fn write(&self, bytes: impl Into<Cow<'static, [u8]>>) {
        self.notifier.notify(bytes);
    }

    /// Cola texto. Usa bracketed paste quando o programa pede, como o Claude Code.
    pub fn paste(&self, text: &str) {
        let bracketed = self.term.lock().mode().contains(TermMode::BRACKETED_PASTE);
        let text = text.replace("\r\n", "\r").replace('\n', "\r");
        if bracketed {
            let clean = text.replace('\x1b', "");
            self.write(format!("\x1b[200~{clean}\x1b[201~").into_bytes());
        } else {
            self.write(text.into_bytes());
        }
        self.term.lock().scroll_display(Scroll::Bottom);
    }

    pub fn set_theme(&self, theme: TerminalTheme) {
        *self.shared.theme.lock().unwrap() = theme;
    }

    pub fn is_exited(&self) -> bool {
        self.shared.exited.load(Ordering::Relaxed)
    }

    pub fn title(&self) -> Option<String> {
        self.shared.title.lock().unwrap().clone()
    }

    pub fn mode(&self) -> TermMode {
        *self.term.lock().mode()
    }

    pub fn grid_size(&self) -> GridSize {
        self.size
    }

    pub(crate) fn resize(&mut self, cols: usize, lines: usize, cell_w: f32, cell_h: f32) {
        let new = GridSize {
            cols: cols.max(2),
            lines: lines.max(1),
            cell_w,
            cell_h,
        };
        if new == self.size {
            return;
        }
        self.size = new;
        *self.shared.size.lock().unwrap() = Some(new.window_size());
        self.notifier.on_resize(new.window_size());
        self.term.lock().resize(new);
    }

    pub fn scroll(&self, delta: i32) {
        if delta == 0 {
            return;
        }
        let mut term = self.term.lock();
        if term
            .mode()
            .contains(TermMode::ALT_SCREEN | TermMode::ALTERNATE_SCROLL)
        {
            let key = if delta > 0 { b'A' } else { b'B' };
            let seq: Vec<u8> = (0..delta.abs()).flat_map(|_| [0x1b, b'O', key]).collect();
            drop(term);
            self.write(seq);
        } else {
            term.scroll_display(Scroll::Delta(delta));
        }
    }

    pub fn scroll_to_bottom(&self) {
        self.term.lock().scroll_display(Scroll::Bottom);
    }

    /// Converte posição em pixels (relativa ao widget) em ponto da grade.
    pub(crate) fn point_at(&self, x: f32, y: f32, display_offset: usize) -> Point {
        let col = ((x / self.size.cell_w).max(0.0) as usize).min(self.size.cols - 1);
        let line = ((y / self.size.cell_h).max(0.0) as usize).min(self.size.lines - 1);
        viewport_to_point(display_offset, Point::new(line, Column(col)))
    }

    fn side_at(&self, x: f32) -> Side {
        if x % self.size.cell_w > self.size.cell_w / 2.0 {
            Side::Right
        } else {
            Side::Left
        }
    }

    pub(crate) fn select_start(&self, ty: SelectionType, x: f32, y: f32) {
        let mut term = self.term.lock();
        let p = self.point_at(x, y, term.grid().display_offset());
        term.selection = Some(Selection::new(ty, p, self.side_at(x)));
    }

    pub(crate) fn select_update(&self, x: f32, y: f32) {
        let mut term = self.term.lock();
        let p = self.point_at(x, y, term.grid().display_offset());
        let side = self.side_at(x);
        if let Some(sel) = term.selection.as_mut() {
            sel.update(p, side);
        }
    }

    pub fn clear_selection(&self) {
        self.term.lock().selection = None;
    }

    pub fn selection_text(&self) -> Option<String> {
        self.term
            .lock()
            .selection_to_string()
            .filter(|s| !s.is_empty())
    }

    /// Últimas `n` linhas não vazias do que está na tela (prévia nos cards).
    pub fn tail_lines(&self, n: usize) -> Vec<String> {
        let term = self.term.lock();
        let grid = term.grid();
        let mut out = Vec::new();
        let cols = grid.columns();
        for line in (0..grid.screen_lines() as i32).rev() {
            let row = &grid[Line(line)];
            let s: String = (0..cols).map(|c| row[Column(c)].c).collect();
            let s = s.trim_end().to_string();
            if !s.trim().is_empty() {
                out.push(s);
                if out.len() == n {
                    break;
                }
            }
        }
        out.reverse();
        out
    }

    /// Todo o scrollback como texto (cache estilo tmux).
    pub fn dump_text(&self, max_lines: usize) -> String {
        let term = self.term.lock();
        let grid = term.grid();
        let cols = grid.columns();
        let top = -(grid.history_size() as i32);
        let bottom = grid.screen_lines() as i32;
        let start = (bottom - max_lines as i32).max(top);
        let mut out = String::new();
        for line in start..bottom {
            let row = &grid[Line(line)];
            let s: String = (0..cols).map(|c| row[Column(c)].c).collect();
            out.push_str(s.trim_end());
            out.push('\n');
        }
        out
    }

    pub(crate) fn hover_link(&mut self, point: Point) {
        let term = self.term.lock();
        let found =
            visible_matches(&term, &mut self.url_regex.clone()).find(|m| m.contains(&point));
        drop(term);
        self.hovered_link = found;
    }

    pub(crate) fn open_hovered_link(&self) {
        let Some(m) = &self.hovered_link else { return };
        let term = self.term.lock();
        let grid = term.grid();
        let mut url = String::new();
        for cell in grid.iter_from(*m.start()) {
            url.push(cell.c);
            if cell.point == *m.end() {
                break;
            }
        }
        // iter_from começa depois do ponto inicial
        let first = grid[*m.start()].c;
        let url = format!("{first}{url}");
        drop(term);
        let _ = open::that_detached(url);
    }

    pub(crate) fn mouse_report(&self, button: u8, point: Point, pressed: bool) {
        let mode = self.mode();
        if mode.contains(TermMode::SGR_MOUSE) {
            let c = if pressed { 'M' } else { 'm' };
            let msg = format!(
                "\x1b[<{};{};{}{}",
                button,
                point.column.0 + 1,
                point.line.0 + 1,
                c
            );
            self.write(msg.into_bytes());
        } else {
            let b = if pressed { button } else { 3 };
            if point.line.0 >= 223 || point.column.0 >= 223 {
                return;
            }
            self.write(vec![
                0x1b,
                b'[',
                b'M',
                32 + b,
                32 + 1 + point.column.0 as u8,
                32 + 1 + point.line.0 as u8,
            ]);
        }
    }

    /// Mata o processo (SIGHUP no grupo, como um terminal fechando).
    pub fn kill(&self) {
        let _ = self.notifier.0.send(Msg::Shutdown);
    }
}

impl Drop for TerminalBackend {
    fn drop(&mut self) {
        self.kill();
    }
}

/// Copiado de alacritty/src/display/hint.rs: matches visíveis na tela.
fn visible_matches<'a>(
    term: &'a Term<EventProxy>,
    regex: &'a mut RegexSearch,
) -> impl Iterator<Item = Match> + 'a {
    let start_line = Line(-(term.grid().display_offset() as i32));
    let end_line = start_line + term.bottommost_line();
    let mut start = term.line_search_left(Point::new(start_line, Column(0)));
    let mut end = term.line_search_right(Point::new(end_line, Column(0)));
    start.line = start.line.max(start_line - 100);
    end.line = end.line.min(end_line + 100);
    RegexIter::new(start, end, Direction::Right, term, regex)
        .skip_while(move |m| m.end().line < start_line)
        .take_while(move |m| m.start().line <= end_line)
}
