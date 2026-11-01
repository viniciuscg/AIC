//! Widget egui que desenha o terminal e manda teclado/mouse pro PTY.

use std::sync::atomic::Ordering;
use std::time::Duration;

use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::index::Point;
use alacritty_terminal::selection::SelectionType;
use alacritty_terminal::term::cell::Flags;
use alacritty_terminal::term::search::Match;
use alacritty_terminal::term::{Term, TermMode};
use alacritty_terminal::vte::ansi::{Color, CursorShape};
use egui::epaint::text::FontsView;
use egui::{
    Color32, CornerRadius, EventFilter, FontId, Id, Key, MouseWheelUnit, PointerButton, Pos2, Rect,
    Response, Sense, Shape, Stroke, StrokeKind, Ui, Vec2,
};

use crate::backend::{EventProxy, TerminalBackend};
use crate::bindings::{BindingAction, BindingsLayout, InputKind};
use crate::theme::TerminalTheme;

/// Fontes do terminal. `bold` é opcional porque o egui não faz negrito sintético.
#[derive(Clone, Debug, PartialEq)]
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
            padding: 3.0,
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
        // só o que interessa ao teclado; frame sem tecla não copia nada nem toca no terminal
        let events: Vec<egui::Event> = ui.input(|i| {
            i.events
                .iter()
                .filter(|e| {
                    matches!(
                        e,
                        egui::Event::Text(_)
                            | egui::Event::Key { pressed: true, .. }
                            | egui::Event::Copy
                            | egui::Event::Cut
                            | egui::Event::Paste(_)
                    )
                })
                .cloned()
                .collect()
        });
        if events.is_empty() {
            return;
        }
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
        let hovered = response.hovered() || response.dragged();

        if let Some(pos) = ui.input(|i| i.pointer.latest_pos()) {
            if inner.contains(pos) || state.dragging {
                let rel = pos - inner.min;
                let display_offset = self.backend.display_offset_fast();
                state.mouse_point = Some(self.backend.point_at(rel.x, rel.y, display_offset));
            }
        }

        if !hovered {
            return;
        }
        let mode = self.backend.mode_fast();
        let mouse_mode = mode.intersects(TermMode::MOUSE_MODE);
        // A roda sobre o terminal é do terminal: zera o delta pra ScrollArea em volta
        // (lista de blocos) não rolar a tela junto.
        ui.ctx().input_mut(|i| i.smooth_scroll_delta = Vec2::ZERO);

        let events: Vec<egui::Event> = ui.input(|i| {
            i.events
                .iter()
                .filter(|e| {
                    matches!(
                        e,
                        egui::Event::MouseWheel { .. }
                            | egui::Event::PointerButton { .. }
                            | egui::Event::PointerMoved(_)
                    )
                })
                .cloned()
                .collect()
        });
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
                        self.backend.touch();
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

    /// Reaproveita o último desenho quando nada mudou; quando mudou, relê a grade e refaz só as
    /// linhas diferentes. Com o PTY segurando o lock, desenha o que já tem em vez de esperar.
    fn paint(&mut self, ui: &Ui, rect: Rect, inner: Rect, cell: Vec2) {
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, CornerRadius::ZERO, self.theme.background);

        let pass = ui.ctx().cumulative_pass_nr();
        let fill = ui.fonts_mut(|f| f.font_atlas_fill_ratio());
        let layout = LayoutKey {
            inner,
            cell,
            pixels_per_point: ui.ctx().pixels_per_point(),
            font: self.font.clone(),
            theme: self.theme.clone(),
        };
        let focused = self.focused;
        let theme = self.theme;
        let font = self.font;
        let b = &mut *self.backend;
        // lida ANTES da grade: saída que chegar durante a leitura redesenha no próximo frame
        let generation = b.shared.generation.load(Ordering::Acquire);
        let cache = &mut b.render;
        // Galley guardado aponta pra posição no atlas de fontes. O egui recria o atlas quando ele
        // enche: se o terminal ficou sem desenhar ou o atlas encolheu, o guardado não vale mais.
        if cache.layout.as_ref() != Some(&layout)
            || pass > cache.last_pass + 1
            || fill < cache.last_fill
        {
            cache.clear();
            cache.layout = Some(layout);
        }
        cache.last_pass = pass;
        cache.last_fill = fill;

        if cache.frame != Some((generation, focused)) {
            let guard = if cache.rows.is_empty() || cache.lock_misses >= MAX_LOCK_MISSES {
                Some(b.term.lock())
            } else {
                b.term.try_lock_unfair()
            };
            match guard {
                Some(term) => {
                    cache.lock_misses = 0;
                    let dirty = cache.scan(&term, theme, font, focused, b.hovered_link.as_ref());
                    drop(term);
                    if !dirty.is_empty() {
                        ui.fonts_mut(|fonts| {
                            for (row, cells) in dirty {
                                let y = inner.min.y + row as f32 * cell.y;
                                cache.rows[row].shapes =
                                    build_row(&cells, inner.min.x, y, cell, theme, font, fonts);
                            }
                        });
                    }
                    cache.frame = Some((generation, focused));
                }
                None => {
                    cache.lock_misses += 1;
                    ui.ctx().request_repaint_after(LOCK_RETRY);
                }
            }
        }

        // texto por cima de tudo, como sempre: o fundo de uma linha invade meio pixel a de baixo
        painter.extend(
            cache
                .rows
                .iter()
                .flat_map(|r| r.shapes.under.iter().cloned()),
        );
        painter.extend(
            cache
                .rows
                .iter()
                .flat_map(|r| r.shapes.text.iter().cloned()),
        );
    }
}

/// Tentativas sem conseguir o lock antes de esperar por ele (sob saída pesada, o PTY quase não solta).
const MAX_LOCK_MISSES: u8 = 8;
const LOCK_RETRY: Duration = Duration::from_millis(16);

/// O que invalida o desenho inteiro: posição, célula, fonte, escala e cores.
#[derive(PartialEq)]
struct LayoutKey {
    inner: Rect,
    cell: Vec2,
    pixels_per_point: f32,
    font: TerminalFont,
    theme: TerminalTheme,
}

/// Formas prontas de uma linha.
#[derive(Default)]
struct RowShapes {
    /// Fundos, cursor e sublinhados, embaixo do texto.
    under: Vec<Shape>,
    text: Vec<Shape>,
}

#[derive(Default)]
struct Row {
    hash: u64,
    shapes: RowShapes,
}

/// O último desenho do terminal, por linha.
#[derive(Default)]
pub(crate) struct RenderCache {
    layout: Option<LayoutKey>,
    /// `(generation, focused)` do que está em `rows`.
    frame: Option<(u64, bool)>,
    rows: Vec<Row>,
    last_pass: u64,
    last_fill: f32,
    lock_misses: u8,
    /// Vetor reaproveitado da leitura da linha.
    scratch: Vec<RowCell>,
}

/// Como o cursor aparece numa célula.
#[derive(Clone, Copy, PartialEq, Eq)]
enum CursorPaint {
    None,
    Block,
    Beam,
    Underline,
    Hollow,
}

/// Uma célula já resolvida: tudo que influencia o desenho, nada mais.
struct RowCell {
    col: u16,
    ch: char,
    zerowidth: Option<Box<[char]>>,
    fg: Color32,
    bg: Color32,
    wide: bool,
    bold: bool,
    underline: bool,
    strike: bool,
    cursor: CursorPaint,
}

impl RowCell {
    fn hash_into(&self, h: u64) -> u64 {
        let flags = self.wide as u64
            | (self.bold as u64) << 1
            | (self.underline as u64) << 2
            | (self.strike as u64) << 3
            | (self.cursor as u64) << 4;
        let mut h = mix(h, self.col as u64 | (self.ch as u64) << 16 | flags << 40);
        h = mix(
            h,
            u32::from_ne_bytes(self.fg.to_array()) as u64
                | (u32::from_ne_bytes(self.bg.to_array()) as u64) << 32,
        );
        if let Some(zw) = &self.zerowidth {
            for c in zw {
                h = mix(h, *c as u64);
            }
        }
        h
    }
}

/// Hash rápido (FxHash): roda pra cada célula da tela quando chega saída.
#[inline]
fn mix(h: u64, v: u64) -> u64 {
    (h.rotate_left(5) ^ v).wrapping_mul(0x517c_c1b7_2722_0a95)
}

impl RenderCache {
    fn clear(&mut self) {
        self.rows.clear();
        self.frame = None;
    }

    /// Percorre a grade e devolve as linhas que mudaram, com as células delas.
    fn scan(
        &mut self,
        term: &Term<EventProxy>,
        theme: &TerminalTheme,
        font: &TerminalFont,
        focused: bool,
        link: Option<&Match>,
    ) -> Vec<(usize, Vec<RowCell>)> {
        let content = term.renderable_content();
        let colors = content.colors;
        let offset = content.display_offset as i32;
        let selection = content.selection;
        let cursor = content.cursor;
        let show_cursor =
            content.mode.contains(TermMode::SHOW_CURSOR) && cursor.shape != CursorShape::Hidden;
        let lines = term.screen_lines();
        if self.rows.len() != lines {
            self.rows.resize_with(lines, Row::default);
        }

        let mut dirty = Vec::new();
        let mut scratch = std::mem::take(&mut self.scratch);
        scratch.clear();
        let mut hash = 0u64;
        let mut current = 0usize;
        let mut finish =
            |row: usize, hash: u64, cells: &mut Vec<RowCell>, rows: &mut Vec<Row>| match rows
                .get_mut(row)
            {
                Some(r) if r.hash != hash || r.hash == 0 => {
                    r.hash = hash;
                    let cap = cells.capacity();
                    dirty.push((row, std::mem::replace(cells, Vec::with_capacity(cap))));
                }
                _ => cells.clear(),
            };

        for indexed in content.display_iter {
            let c = &indexed.cell;
            if c.flags.contains(Flags::WIDE_CHAR_SPACER) {
                continue;
            }
            let point = indexed.point;
            let row = (point.line.0 + offset) as usize;
            if row != current {
                finish(current, hash, &mut scratch, &mut self.rows);
                current = row;
                hash = 0;
            }

            let mut fg = theme.resolve(c.fg, colors);
            let mut bg = theme.resolve(c.bg, colors);
            if c.flags.contains(Flags::BOLD)
                && let Color::Named(n) = c.fg
                && (n as usize) < 8
            {
                fg = theme.ansi[n as usize + 8];
            }
            if c.flags.intersects(Flags::DIM | Flags::DIM_BOLD) {
                fg = fg.gamma_multiply(0.66);
            }
            if c.flags.contains(Flags::INVERSE) {
                std::mem::swap(&mut fg, &mut bg);
            }
            if selection.is_some_and(|s| s.contains(point)) {
                bg = theme.selection;
            }
            if c.flags.contains(Flags::HIDDEN) {
                fg = bg;
            }
            let cursor_paint = if show_cursor && point == cursor.point {
                match cursor.shape {
                    CursorShape::Block if focused => {
                        fg = theme.background;
                        CursorPaint::Block
                    }
                    CursorShape::Beam => CursorPaint::Beam,
                    CursorShape::Underline => CursorPaint::Underline,
                    _ => CursorPaint::Hollow,
                }
            } else {
                CursorPaint::None
            };
            let cell = RowCell {
                col: point.column.0 as u16,
                ch: c.c,
                zerowidth: c.zerowidth().map(Box::from),
                fg,
                bg,
                wide: c.flags.contains(Flags::WIDE_CHAR),
                bold: c.flags.contains(Flags::BOLD) && font.bold.is_some(),
                underline: c.flags.intersects(Flags::ALL_UNDERLINES)
                    || link.is_some_and(|l| l.contains(&point)),
                strike: c.flags.contains(Flags::STRIKEOUT),
                cursor: cursor_paint,
            };
            hash = cell.hash_into(hash);
            scratch.push(cell);
        }
        finish(current, hash, &mut scratch, &mut self.rows);
        self.scratch = scratch;
        dirty
    }
}

/// Monta as formas de uma linha: fundos vizinhos viram um retângulo só e texto vira galley pronto.
fn build_row(
    cells: &[RowCell],
    left: f32,
    y: f32,
    cell: Vec2,
    theme: &TerminalTheme,
    font: &TerminalFont,
    fonts: &mut FontsView<'_>,
) -> RowShapes {
    let mut out = RowShapes::default();
    let x_of = |col: u16| left + col as f32 * cell.x;

    // fundos: mesma cor em células vizinhas = um retângulo
    let mut bg_run: Option<(u16, u16, Color32)> = None;
    let flush_bg = |run: &mut Option<(u16, u16, Color32)>, out: &mut RowShapes| {
        if let Some((start, end, color)) = run.take() {
            out.under.push(Shape::rect_filled(
                Rect::from_min_size(
                    Pos2::new(x_of(start), y),
                    Vec2::new((end - start) as f32 * cell.x + 0.5, cell.y + 0.5),
                ),
                CornerRadius::ZERO,
                color,
            ));
        }
    };
    for c in cells {
        let end = c.col + if c.wide { 2 } else { 1 };
        if c.bg == theme.background {
            flush_bg(&mut bg_run, &mut out);
            continue;
        }
        match &mut bg_run {
            Some((_, e, color)) if *e == c.col && *color == c.bg => *e = end,
            _ => {
                flush_bg(&mut bg_run, &mut out);
                bg_run = Some((c.col, end, c.bg));
            }
        }
    }
    flush_bg(&mut bg_run, &mut out);

    // Texto em "runs" (mesma cor/estilo em células seguidas) = bem menos formas por frame.
    struct Run {
        x: f32,
        text: String,
        color: Color32,
        bold: bool,
        end_col: u16,
    }
    let mut run: Option<Run> = None;
    let flush_text = |run: &mut Option<Run>, out: &mut RowShapes, fonts: &mut FontsView<'_>| {
        if let Some(r) = run.take()
            && !r.text.trim().is_empty()
        {
            let f = match (&font.bold, r.bold) {
                (Some(b), true) => b.clone(),
                _ => font.regular.clone(),
            };
            // espaço no fim da linha não desenha nada, só encarece o layout
            let mut text = r.text;
            text.truncate(text.trim_end().len());
            let g = fonts.layout_no_wrap(text, f, r.color);
            out.text.push(Shape::galley(Pos2::new(r.x, y), g, r.color));
        }
    };

    for c in cells {
        let x = x_of(c.col);
        let w = if c.wide { cell.x * 2.0 } else { cell.x };
        let r = Rect::from_min_size(Pos2::new(x, y), Vec2::new(w, cell.y));

        if c.cursor == CursorPaint::Block {
            out.under
                .push(Shape::rect_filled(r, CornerRadius::ZERO, theme.cursor));
        }
        if c.underline {
            let uy = y + cell.y - 1.0;
            out.under.push(Shape::line_segment(
                [Pos2::new(x, uy), Pos2::new(x + w, uy)],
                Stroke::new(1.0, c.fg),
            ));
        }
        if c.strike {
            let sy = y + cell.y / 2.0;
            out.under.push(Shape::line_segment(
                [Pos2::new(x, sy), Pos2::new(x + w, sy)],
                Stroke::new(1.0, c.fg),
            ));
        }

        // Caracteres fora do "seguro" (ícones, emoji, CJK) vão sozinhos, centralizados
        // na célula, para não desalinhar a linha se a fonte de fallback for mais larga.
        let ch = c.ch;
        let simple = !c.wide
            && (ch.is_ascii()
                || ('\u{a0}'..='\u{24ff}').contains(&ch)
                || ('\u{2500}'..='\u{259f}').contains(&ch));
        let continues = run
            .as_ref()
            .is_some_and(|t| t.color == c.fg && t.bold == c.bold && t.end_col == c.col);
        if !(simple && continues) {
            flush_text(&mut run, &mut out, fonts);
        }
        if simple {
            let t = run.get_or_insert_with(|| Run {
                x,
                text: String::new(),
                color: c.fg,
                bold: c.bold,
                end_col: c.col,
            });
            t.text.push(if ch == '\t' { ' ' } else { ch });
            if let Some(zw) = &c.zerowidth {
                t.text.extend(zw.iter());
            }
            t.end_col = c.col + 1;
        } else if ch != ' ' {
            let g = fonts.layout_no_wrap(ch.to_string(), font.regular.clone(), c.fg);
            let pos = Pos2::new(x + w / 2.0 - g.size().x / 2.0, y);
            out.text.push(Shape::galley(pos, g, c.fg));
        }

        match c.cursor {
            CursorPaint::Beam => out.under.push(Shape::rect_filled(
                Rect::from_min_size(r.min, Vec2::new(2.0, cell.y)),
                CornerRadius::ZERO,
                theme.cursor,
            )),
            CursorPaint::Underline => out.under.push(Shape::rect_filled(
                Rect::from_min_size(Pos2::new(r.min.x, r.max.y - 2.0), Vec2::new(w, 2.0)),
                CornerRadius::ZERO,
                theme.cursor,
            )),
            CursorPaint::Hollow => out.under.push(Shape::rect_stroke(
                r.shrink(0.5),
                CornerRadius::ZERO,
                Stroke::new(1.0, theme.cursor),
                StrokeKind::Inside,
            )),
            CursorPaint::None | CursorPaint::Block => {}
        }
    }
    flush_text(&mut run, &mut out, fonts);
    out
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

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::AtomicUsize;

    use egui::epaint::TextShape;
    use egui::{Context, Pos2, RawInput, Shape, Vec2};

    use super::*;
    use crate::backend::{TermSettings, set_window_focused};
    use crate::bindings::BindingsLayout;

    fn spawn(ctx: &Context, script: &str) -> TerminalBackend {
        let settings = TermSettings {
            program: "bash".into(),
            args: vec!["-c".into(), script.into()],
            ..Default::default()
        };
        TerminalBackend::spawn(1, ctx.clone(), settings, TerminalTheme::default()).unwrap()
    }

    fn frame(ctx: &Context, t: &mut TerminalBackend) {
        let theme = TerminalTheme::default();
        let font = TerminalFont::default();
        let bindings = BindingsLayout::new();
        let raw = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 400.0))),
            ..Default::default()
        };
        let mut out = ctx.run_ui(raw, |ui| {
            TerminalView::new(t, &theme, &font, &bindings).show(ui, Vec2::new(800.0, 400.0));
        });
        out.textures_delta.clear();
    }

    fn wait(
        ctx: &Context,
        t: &mut TerminalBackend,
        what: &str,
        f: impl Fn(&TerminalBackend) -> bool,
    ) {
        for _ in 0..200 {
            frame(ctx, t);
            if f(t) {
                return;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        panic!("esperando: {what}");
    }

    fn galley(s: &Shape) -> Arc<egui::Galley> {
        match s {
            Shape::Text(TextShape { galley, .. }) => galley.clone(),
            other => panic!("não é texto: {other:?}"),
        }
    }

    fn row_text(t: &TerminalBackend, row: usize) -> &[Shape] {
        &t.render.rows[row].shapes.text
    }

    #[test]
    fn linha_inalterada_reaproveita_as_formas() {
        let ctx = Context::default();
        let mut t = spawn(
            &ctx,
            "printf 'fixa\\n'; sleep 0.4; printf 'nova\\n'; sleep 30",
        );
        wait(&ctx, &mut t, "primeira linha", |t| {
            t.render
                .rows
                .first()
                .is_some_and(|r| !r.shapes.text.is_empty())
        });
        let before = galley(&row_text(&t, 0)[0]);
        wait(&ctx, &mut t, "segunda linha", |t| {
            !row_text(t, 1).is_empty()
        });
        let after = galley(&row_text(&t, 0)[0]);
        assert!(Arc::ptr_eq(&before, &after), "a linha 0 não mudou");
        assert_eq!(after.job.text, "fixa");
    }

    #[test]
    fn nada_mudou_nao_relê_a_grade() {
        let ctx = Context::default();
        let mut t = spawn(&ctx, "printf 'oi\\n'; sleep 30");
        wait(&ctx, &mut t, "texto", |t| {
            t.render
                .rows
                .first()
                .is_some_and(|r| !r.shapes.text.is_empty())
        });
        std::thread::sleep(Duration::from_millis(100));
        frame(&ctx, &mut t);
        let key = t.render.frame;
        assert_eq!(
            key.map(|k| k.0),
            Some(t.shared.generation.load(Ordering::Acquire))
        );
        // marca a linha: se a grade fosse relida com a chave igual, o hash voltaria ao certo
        t.render.rows[0].hash = 42;
        frame(&ctx, &mut t);
        assert_eq!(t.render.rows[0].hash, 42);
    }

    #[test]
    fn fundos_vizinhos_viram_um_retangulo() {
        let ctx = Context::default();
        let mut t = spawn(&ctx, "printf '\\033[44m      \\033[0m x\\n'; sleep 30");
        wait(&ctx, &mut t, "texto", |t| {
            t.render
                .rows
                .first()
                .is_some_and(|r| !r.shapes.text.is_empty())
        });
        let rects: Vec<_> = t.render.rows[0]
            .shapes
            .under
            .iter()
            .filter_map(|s| match s {
                Shape::Rect(r) => Some(r.rect),
                _ => None,
            })
            .collect();
        assert_eq!(rects.len(), 1, "um retângulo pras 6 células azuis");
        let cell = t.size.cell_w;
        assert!((rects[0].width() - (6.0 * cell + 0.5)).abs() < 0.01);
    }

    #[test]
    fn invisivel_nao_pede_redesenho() {
        let ctx = Context::default();
        let requests = Arc::new(AtomicUsize::new(0));
        let r = requests.clone();
        let mut t = spawn(
            &ctx,
            "sleep 0.3; for i in 1 2 3 4 5; do echo $i; sleep 0.05; done; sleep 30",
        );
        t.set_visible(false);
        // o egui pede redesenho nos primeiros frames, e o callback só avisa do que for mais cedo
        for _ in 0..5 {
            frame(&ctx, &mut t);
        }
        ctx.set_request_repaint_callback(move |_| {
            r.fetch_add(1, Ordering::Relaxed);
        });
        let g0 = t.shared.generation.load(Ordering::Acquire);
        std::thread::sleep(Duration::from_millis(900));
        assert!(
            t.shared.generation.load(Ordering::Acquire) > g0,
            "a saída chegou e marcou sujo"
        );
        assert_eq!(requests.load(Ordering::Relaxed), 0);

        // um terminal visível, com a mesma saída, pede
        let _visible = spawn(&ctx, "sleep 0.2; echo oi; sleep 30");
        std::thread::sleep(Duration::from_millis(600));
        assert!(requests.load(Ordering::Relaxed) > 0);
    }

    #[test]
    fn saida_com_sucesso_e_clean_exit() {
        let ctx = Context::default();
        let t = spawn(&ctx, "exit 0");
        let bad = spawn(&ctx, "exit 3");
        for _ in 0..200 {
            if t.is_exited() && bad.is_exited() {
                break;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        assert!(t.clean_exit());
        assert!(!bad.clean_exit());
        set_window_focused(false);
        set_window_focused(true);
    }
}
