//! Widget egui que desenha o terminal e manda teclado/mouse pro PTY.

use alacritty_terminal::index::Point;
use alacritty_terminal::selection::SelectionType;
use alacritty_terminal::term::TermMode;
use alacritty_terminal::term::cell::Flags;
use alacritty_terminal::vte::ansi::{Color, CursorShape};
use egui::{
    Align2, Color32, CornerRadius, EventFilter, FontId, Id, Key, MouseWheelUnit, PointerButton,
    Pos2, Rect, Response, Sense, Stroke, StrokeKind, Ui, Vec2,
};

use crate::backend::TerminalBackend;
use crate::bindings::{BindingAction, BindingsLayout, InputKind};
use crate::theme::TerminalTheme;

/// Fontes do terminal. `bold` é opcional porque o egui não faz negrito sintético.
#[derive(Clone, Debug)]
pub struct TerminalFont {
    pub regular: FontId,
    pub bold: Option<FontId>,
}

impl Default for TerminalFont {
    fn default() -> Self {
        Self {
            regular: FontId::monospace(14.0),
            bold: None,
        }
    }
}

#[derive(Clone, Default)]
struct ViewState {
    dragging: bool,
    scroll_px: f32,
    mouse_point: Option<Point>,
    /// Último ponto reportado em movimento (evita mandar o mesmo ponto várias vezes).
    motion_point: Option<Point>,
}

pub struct TerminalView<'a> {
    backend: &'a mut TerminalBackend,
    theme: &'a TerminalTheme,
    font: &'a TerminalFont,
    bindings: &'a BindingsLayout,
    focused: bool,
    padding: f32,
}

impl<'a> TerminalView<'a> {
    pub fn new(
        backend: &'a mut TerminalBackend,
        theme: &'a TerminalTheme,
        font: &'a TerminalFont,
        bindings: &'a BindingsLayout,
    ) -> Self {
        Self {
            backend,
            theme,
            font,
            bindings,
            focused: false,
            padding: 6.0,
        }
    }

    pub fn focused(mut self, focused: bool) -> Self {
        self.focused = focused;
        self
    }

    pub fn padding(mut self, padding: f32) -> Self {
        self.padding = padding;
        self
    }

    /// Tamanho de uma célula (largura do 'M', altura da linha) para a fonte.
    pub fn cell_size(ui: &Ui, font: &FontId) -> Vec2 {
        ui.fonts_mut(|f| Vec2::new(f.glyph_width(font, 'M'), f.row_height(font)))
    }

    pub fn show(mut self, ui: &mut Ui, size: Vec2) -> Response {
        let (rect, response) = ui.allocate_exact_size(size, Sense::click_and_drag());
        let id = Id::new(("aic-term", self.backend.id));
        let mut state: ViewState = ui.memory(|m| m.data.get_temp(id).unwrap_or_default());

        let cell = Self::cell_size(ui, &self.font.regular);
        let inner = rect.shrink(self.padding);
        let cols = (inner.width() / cell.x).floor() as usize;
        let lines = (inner.height() / cell.y).floor() as usize;
        self.backend.resize(cols, lines, cell.x, cell.y);

        if self.focused {
            response.request_focus();
            ui.memory_mut(|m| {
                m.set_focus_lock_filter(
                    response.id,
                    EventFilter {
                        tab: true,
                        horizontal_arrows: true,
                        vertical_arrows: true,
                        escape: true,
                    },
                )
            });
            self.keyboard(ui);
        }
        self.mouse(ui, &response, inner, &mut state);
        self.paint(ui, rect, inner, cell);

        ui.memory_mut(|m| m.data.insert_temp(id, state));
        response
    }

    fn keyboard(&self, ui: &Ui) {
        let events = ui.input(|i| i.events.clone());
        let mode = self.backend.mode();
        let mut wrote = false;
        for ev in events {
            match ev {
                egui::Event::Text(text) => {
                    self.backend.write(text.into_bytes());
                    wrote = true;
                }
                egui::Event::Key {
                    key,
                    pressed: true,
                    modifiers,
                    ..
                } => {
                    match self
                        .bindings
                        .get_action(InputKind::KeyCode(key), modifiers, mode)
                    {
                        BindingAction::Char(c) => {
                            self.backend.write(c.to_string().into_bytes());
                            wrote = true;
                        }
                        BindingAction::Esc(seq) => {
                            self.backend.write(seq.into_bytes());
                            wrote = true;
                        }
                        _ => {
                            // Alt+letra → ESC + letra (atalhos de readline/Claude Code)
                            if modifiers.alt && !modifiers.ctrl {
                                if let Some(c) = key_char(key, modifiers.shift) {
                                    self.backend.write(format!("\x1b{c}").into_bytes());
                                    wrote = true;
                                }
                            }
                        }
                    }
                }
                // O egui converte Ctrl+C/V/X em eventos de clipboard. No terminal só
                // Ctrl+Shift copia/cola; sem Shift, o byte de controle vai pro programa
                // (Ctrl+V cru deixa o Claude Code colar imagem do clipboard).
                egui::Event::Copy => {
                    if ui.input(|i| i.modifiers.shift) {
                        if let Some(t) = self.backend.selection_text() {
                            ui.ctx().copy_text(t);
                        }
                    } else {
                        self.backend.write(vec![0x03]);
                        wrote = true;
                    }
                }
                egui::Event::Cut => {
                    self.backend.write(vec![0x18]);
                    wrote = true;
                }
                egui::Event::Paste(text) => {
                    if ui.input(|i| i.modifiers.shift) {
                        self.backend.paste(&text);
                    } else {
                        self.backend.write(vec![0x16]);
                    }
                    wrote = true;
                }
                _ => {}
            }
        }
        if wrote {
            self.backend.clear_selection();
            self.backend.scroll_to_bottom();
        }
    }

    fn mouse(&mut self, ui: &Ui, response: &Response, inner: Rect, state: &mut ViewState) {
        let mode = self.backend.mode();
        let mouse_mode = mode.intersects(TermMode::MOUSE_MODE);
        let hovered = response.hovered() || response.dragged();
        let display_offset = self.backend.term.lock().grid().display_offset();

        if let Some(pos) = ui.input(|i| i.pointer.latest_pos()) {
            if inner.contains(pos) || state.dragging {
                let rel = pos - inner.min;
                state.mouse_point = Some(self.backend.point_at(rel.x, rel.y, display_offset));
            }
        }

        if !hovered {
            return;
        }
        // A roda sobre o terminal é do terminal: zera o delta pra ScrollArea em volta
        // (lista de blocos) não rolar a tela junto.
        ui.ctx().input_mut(|i| i.smooth_scroll_delta = Vec2::ZERO);

        let events = ui.input(|i| i.events.clone());
        let mods = ui.input(|i| i.modifiers);
        for ev in events {
            match ev {
                egui::Event::MouseWheel { unit, delta, .. } => {
                    let lines = match unit {
                        MouseWheelUnit::Point => {
                            state.scroll_px += delta.y;
                            let l = (state.scroll_px / self.backend.size.cell_h).trunc();
                            state.scroll_px -= l * self.backend.size.cell_h;
                            l as i32
                        }
                        MouseWheelUnit::Line => delta.y.round() as i32 * 3,
                        MouseWheelUnit::Page => (delta.y * self.backend.size.lines as f32) as i32,
                    };
                    if lines == 0 {
                        continue;
                    }
                    if mouse_mode && !mods.shift {
                        if let Some(p) = state.mouse_point {
                            let btn = if lines > 0 { 64 } else { 65 };
                            for _ in 0..lines.abs() {
                                self.backend.mouse_report(btn, p, true);
                            }
                        }
                    } else {
                        self.backend.scroll(lines);
                    }
                }
                egui::Event::PointerButton {
                    button: PointerButton::Primary,
                    pressed,
                    pos,
                    modifiers,
                    ..
                } => {
                    if !inner.contains(pos) && pressed {
                        continue;
                    }
                    let rel = pos - inner.min;
                    if mouse_mode && !modifiers.shift {
                        if let Some(p) = state.mouse_point {
                            self.backend.mouse_report(0, p, pressed);
                        }
                        state.dragging = pressed;
                        state.motion_point = state.mouse_point;
                    } else if pressed {
                        if modifiers.command && self.backend.hovered_link.is_some() {
                            self.backend.open_hovered_link();
                            continue;
                        }
                        let ty = if response.triple_clicked() {
                            SelectionType::Lines
                        } else if response.double_clicked() {
                            SelectionType::Semantic
                        } else {
                            SelectionType::Simple
                        };
                        if ty == SelectionType::Simple {
                            self.backend.clear_selection();
                        }
                        self.backend.select_start(ty, rel.x, rel.y);
                        state.dragging = true;
                    } else {
                        state.dragging = false;
                    }
                }
                egui::Event::PointerMoved(pos) => {
                    // Arrastar com o programa pedindo mouse (Claude Code em tela cheia, tmux):
                    // manda o movimento, senão ele só vê clique e soltura e não seleciona nada.
                    if mouse_mode && !mods.shift {
                        let report = if state.dragging {
                            mode.intersects(TermMode::MOUSE_DRAG | TermMode::MOUSE_MOTION)
                                .then_some(32)
                        } else {
                            mode.contains(TermMode::MOUSE_MOTION).then_some(35)
                        };
                        if let (Some(btn), Some(p)) = (report, state.mouse_point) {
                            if state.motion_point != Some(p) {
                                state.motion_point = Some(p);
                                self.backend.mouse_report(btn, p, true);
                            }
                        }
                    }
                    if state.dragging && !mouse_mode {
                        let rel = pos - inner.min;
                        self.backend.select_update(rel.x, rel.y);
                    }
                    if mods.command {
                        if let Some(p) = state.mouse_point {
                            self.backend.hover_link(p);
                        }
                    } else if self.backend.hovered_link.is_some() {
                        self.backend.hovered_link = None;
                    }
                }
                _ => {}
            }
        }
        if self.backend.hovered_link.is_some() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        } else {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Text);
        }
    }

    fn paint(&self, ui: &Ui, rect: Rect, inner: Rect, cell: Vec2) {
        let painter = ui.painter_at(rect);
        let theme = self.theme;
        painter.rect_filled(rect, CornerRadius::ZERO, theme.background);

        let term = self.backend.term.lock();
        let content = term.renderable_content();
        let colors = content.colors;
        let offset = content.display_offset as i32;
        let selection = content.selection;
        let cursor = content.cursor;
        let show_cursor =
            content.mode.contains(TermMode::SHOW_CURSOR) && cursor.shape != CursorShape::Hidden;
        let link = self.backend.hovered_link.clone();

        // Texto em "runs" (mesma cor/estilo na mesma linha) = bem menos shapes por frame.
        struct Run {
            x: f32,
            y: f32,
            text: String,
            color: Color32,
            bold: bool,
            centered: bool,
        }
        let mut runs: Vec<Run> = Vec::with_capacity(256);
        let mut current: Option<Run> = None;
        let mut last_line = i32::MIN;
        let mut last_col_end = 0usize;

        let flush = |current: &mut Option<Run>, runs: &mut Vec<Run>| {
            if let Some(r) = current.take() {
                if !r.text.trim().is_empty() {
                    runs.push(r);
                }
            }
        };

        for indexed in content.display_iter {
            let c = &indexed.cell;
            if c.flags.contains(Flags::WIDE_CHAR_SPACER) {
                continue;
            }
            let point = indexed.point;
            let row = point.line.0 + offset;
            let col = point.column.0;
            let x = inner.min.x + col as f32 * cell.x;
            let y = inner.min.y + row as f32 * cell.y;
            let wide = c.flags.contains(Flags::WIDE_CHAR);
            let w = if wide { cell.x * 2.0 } else { cell.x };

            let mut fg = theme.resolve(c.fg, colors);
            let mut bg = theme.resolve(c.bg, colors);
            if c.flags.contains(Flags::BOLD) {
                if let Color::Named(n) = c.fg {
                    if (n as usize) < 8 {
                        fg = theme.ansi[n as usize + 8];
                    }
                }
            }
            if c.flags.intersects(Flags::DIM | Flags::DIM_BOLD) {
                fg = fg.gamma_multiply(0.66);
            }
            if c.flags.contains(Flags::INVERSE) {
                std::mem::swap(&mut fg, &mut bg);
            }
            let selected = selection.is_some_and(|s| s.contains(point));
            if selected {
                bg = theme.selection;
            }
            if c.flags.contains(Flags::HIDDEN) {
                fg = bg;
            }

            if bg != theme.background {
                painter.rect_filled(
                    Rect::from_min_size(Pos2::new(x, y), Vec2::new(w + 0.5, cell.y + 0.5)),
                    CornerRadius::ZERO,
                    bg,
                );
            }

            let is_cursor = show_cursor && point == cursor.point;
            if is_cursor && self.focused && cursor.shape == CursorShape::Block {
                painter.rect_filled(
                    Rect::from_min_size(Pos2::new(x, y), Vec2::new(w, cell.y)),
                    CornerRadius::ZERO,
                    theme.cursor,
                );
                fg = theme.background;
            }

            if c.flags.intersects(Flags::ALL_UNDERLINES)
                || link.as_ref().is_some_and(|l| l.contains(&point))
            {
                let uy = y + cell.y - 1.0;
                painter.line_segment(
                    [Pos2::new(x, uy), Pos2::new(x + w, uy)],
                    Stroke::new(1.0, fg),
                );
            }
            if c.flags.contains(Flags::STRIKEOUT) {
                let sy = y + cell.y / 2.0;
                painter.line_segment(
                    [Pos2::new(x, sy), Pos2::new(x + w, sy)],
                    Stroke::new(1.0, fg),
                );
            }

            let ch = c.c;
            let bold = c.flags.contains(Flags::BOLD) && self.font.bold.is_some();
            // Caracteres fora do "seguro" (ícones, emoji, CJK) vão sozinhos, centralizados
            // na célula, para não desalinhar a linha se a fonte de fallback for mais larga.
            let simple = !wide
                && (ch.is_ascii()
                    || ('\u{a0}'..='\u{24ff}').contains(&ch)
                    || ('\u{2500}'..='\u{259f}').contains(&ch));

            let continues = current.as_ref().is_some_and(|r| {
                r.color == fg && r.bold == bold && last_line == row && last_col_end == col
            });
            if !(simple && continues) {
                flush(&mut current, &mut runs);
            }
            if simple {
                let r = current.get_or_insert_with(|| Run {
                    x,
                    y,
                    text: String::new(),
                    color: fg,
                    bold,
                    centered: false,
                });
                r.text.push(if ch == '\t' { ' ' } else { ch });
                if let Some(zw) = c.zerowidth() {
                    r.text.extend(zw);
                }
            } else if ch != ' ' {
                runs.push(Run {
                    x: x + w / 2.0,
                    y,
                    text: ch.to_string(),
                    color: fg,
                    bold: false,
                    centered: true,
                });
            }
            last_line = row;
            last_col_end = col + 1;

            if is_cursor && !(self.focused && cursor.shape == CursorShape::Block) {
                let r = Rect::from_min_size(Pos2::new(x, y), Vec2::new(w, cell.y));
                match cursor.shape {
                    CursorShape::Beam => {
                        painter.rect_filled(
                            Rect::from_min_size(r.min, Vec2::new(2.0, cell.y)),
                            CornerRadius::ZERO,
                            theme.cursor,
                        );
                    }
                    CursorShape::Underline => {
                        painter.rect_filled(
                            Rect::from_min_size(
                                Pos2::new(r.min.x, r.max.y - 2.0),
                                Vec2::new(w, 2.0),
                            ),
                            CornerRadius::ZERO,
                            theme.cursor,
                        );
                    }
                    _ => {
                        painter.rect_stroke(
                            r.shrink(0.5),
                            CornerRadius::ZERO,
                            Stroke::new(1.0, theme.cursor),
                            StrokeKind::Inside,
                        );
                    }
                }
            }
        }
        flush(&mut current, &mut runs);
        drop(term);

        for r in runs {
            let font = if r.bold {
                self.font.bold.clone().unwrap()
            } else {
                self.font.regular.clone()
            };
            let align = if r.centered {
                Align2::CENTER_TOP
            } else {
                Align2::LEFT_TOP
            };
            painter.text(Pos2::new(r.x, r.y), align, r.text, font, r.color);
        }
    }
}

fn key_char(key: Key, shift: bool) -> Option<char> {
    let name = key.name();
    let mut chars = name.chars();
    let c = chars.next()?;
    if chars.next().is_some() {
        return match key {
            Key::Backspace => Some('\x7f'),
            Key::Enter => Some('\r'),
            _ => None,
        };
    }
    Some(if shift {
        c.to_ascii_uppercase()
    } else {
        c.to_ascii_lowercase()
    })
}
