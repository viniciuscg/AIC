//! Prefixo estilo tmux: aperta `cfg.prefix` (padrão `Ctrl+B`), solta e aperta a tecla da ação.
//! Não abre menu nenhum, só acende o indicador na barra de cima e troca as dicas do rodapé;
//! quem quer ver tudo usa `Ctrl+B ?` ou a tecla líder (catálogo).
//!
//! - `Ctrl+B Ctrl+B` manda um Ctrl+B de verdade pro terminal em foco;
//! - `Esc` (ou uma tecla sem ação) cancela.
//!
//! Enquanto armado, o prefixo engole todo o teclado: nada vaza pro terminal.

use egui::{Context, Event, Key, Modifiers};

use crate::actions::{consume_exact, format_shortcut, parse_shortcut, same_mods};
use crate::app::{App, ToastKind};

/// Prefixo normalizado ("Ctrl+B"); vazio se desligado ou inválido.
pub fn label(app: &App) -> String {
    parse_shortcut(&app.cfg.prefix).map(|s| format_shortcut(&s)).unwrap_or_default()
}

/// "Ctrl+B c": o acorde de prefixo de uma ação (vazio se ela não tem tecla de prefixo).
pub fn chord(app: &App, a: crate::actions::Action) -> String {
    match (label(app), app.keymap.prefix_keys(a).first()) {
        (p, Some(k)) if !p.is_empty() => format!("{p} {k}"),
        _ => String::new(),
    }
}

/// O que a tecla apertada vira na tabela do prefixo. Texto tem prioridade (respeita o layout do
/// teclado, `(` `$` `%`); teclas sem texto (Tab, setas, Enter) vêm do evento de tecla.
pub(crate) fn token(events: &[Event]) -> Option<Result<String, ()>> {
    let has_text = events.iter().any(|e| matches!(e, Event::Text(t) if !t.is_empty()));
    for e in events {
        match e {
            Event::Text(t) => {
                if let Some(c) = t.chars().next() {
                    return Some(Ok(if c == ' ' { "Space".into() } else { c.to_string() }));
                }
            }
            Event::Key { key, pressed: true, modifiers, .. } => {
                let named = match key {
                    Key::Escape => return Some(Err(())),
                    Key::Tab => Some("Tab"),
                    Key::Enter => Some("Enter"),
                    Key::ArrowLeft => Some("←"),
                    Key::ArrowRight => Some("→"),
                    Key::ArrowUp => Some("↑"),
                    Key::ArrowDown => Some("↓"),
                    Key::PageUp => Some("PageUp"),
                    Key::PageDown => Some("PageDown"),
                    Key::Delete => Some("Delete"),
                    Key::Space if !has_text => Some("Space"),
                    _ => None,
                };
                if let Some(n) = named {
                    return Some(Ok(n.into()));
                }
                // Sem evento de texto (Ctrl/Alt seguro, ou teste): deduz pela tecla lógica.
                if !has_text && !modifiers.ctrl && !modifiers.alt
                    && let Some(c) = key_char(*key, modifiers.shift)
                {
                    return Some(Ok(c.to_string()));
                }
            }
            _ => {}
        }
    }
    None
}

/// Caractere de uma tecla lógica quando não veio texto (o layout já está aplicado no `Key`).
fn key_char(key: Key, shift: bool) -> Option<char> {
    Some(match key {
        Key::Comma => ',',
        Key::Period => '.',
        Key::Minus => '-',
        Key::Plus => '+',
        Key::Equals => '=',
        Key::OpenBracket => '[',
        Key::CloseBracket => ']',
        Key::OpenCurlyBracket => '{',
        Key::CloseCurlyBracket => '}',
        Key::Questionmark => '?',
        Key::Slash if shift => '?',
        Key::Slash => '/',
        Key::Colon => ':',
        Key::Semicolon => ';',
        Key::Backslash => '\\',
        Key::Pipe => '|',
        Key::Exclamationmark => '!',
        k => {
            let name = k.name();
            let c = name.chars().next().filter(|_| name.chars().count() == 1)?;
            if !c.is_ascii_alphanumeric() {
                return None;
            }
            if shift { c.to_ascii_uppercase() } else { c.to_ascii_lowercase() }
        }
    })
}

/// Trata o teclado do prefixo. Devolve `true` se o prefixo tomou o teclado neste frame.
pub fn handle(app: &mut App, ctx: &Context) -> bool {
    let Some(sc) = parse_shortcut(&app.cfg.prefix) else {
        app.prefix_pending = false;
        return false;
    };
    if !app.prefix_pending {
        if ctx.input_mut(|i| consume_exact(i, sc.modifiers, sc.logical_key)) {
            app.prefix_pending = true;
            app.leader = None;
            // o resto deste frame também não vai pro terminal
            ctx.input_mut(|i| i.events.retain(|e| !matches!(e, Event::Key { .. } | Event::Text(_))));
            return true;
        }
        return false;
    }

    // armado: engole tudo e procura a tecla da ação
    let events: Vec<Event> = ctx.input_mut(|i| {
        let mut out = vec![];
        i.events.retain(|e| match e {
            Event::Key { .. } | Event::Text(_) | Event::Paste(_) => {
                out.push(e.clone());
                false
            }
            _ => true,
        });
        out
    });
    // prefixo de novo: manda o byte de controle pro terminal (C-b = 0x02)
    let again = events.iter().any(|e| matches!(e, Event::Key { key, pressed: true, modifiers, .. } if *key == sc.logical_key && same_mods(*modifiers, sc.modifiers)));
    if again {
        app.prefix_pending = false;
        send_prefix_to_terminal(app, sc.logical_key, sc.modifiers);
        return true;
    }
    let Some(tok) = token(&events) else { return true };
    app.prefix_pending = false;
    let Ok(tok) = tok else { return true };
    match app.keymap.prefix_action(&tok) {
        Some(a) => app.run_action(ctx, a),
        None => {
            let p = label(app);
            app.toast(&format!("{p} {tok} não faz nada · {p} ? mostra todos os atalhos"), ToastKind::Info);
        }
    }
    true
}

/// `Ctrl+letra` vira o byte de controle correspondente; outras combinações não têm byte.
fn send_prefix_to_terminal(app: &mut App, key: Key, mods: Modifiers) {
    let name = key.name();
    let byte = (mods.ctrl && !mods.alt && name.len() == 1)
        .then(|| name.chars().next())
        .flatten()
        .filter(|c| c.is_ascii_alphabetic())
        .map(|c| (c.to_ascii_uppercase() as u8) - b'@');
    let Some(byte) = byte else { return };
    if let Some(t) = app.focused().and_then(|id| app.sessions.get(&id)).and_then(|r| r.term.as_ref()) {
        t.write(vec![byte]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(k: Key, m: Modifiers) -> Event {
        Event::Key { key: k, physical_key: None, pressed: true, repeat: false, modifiers: m }
    }

    #[test]
    fn texto_vence_a_tecla() {
        // Shift+9 no layout ABNT/US vira "(": o texto diz o que foi digitado
        let ev = vec![key(Key::Num9, Modifiers::SHIFT), Event::Text("(".into())];
        assert_eq!(token(&ev), Some(Ok("(".into())));
    }

    #[test]
    fn teclas_sem_texto() {
        assert_eq!(token(&[key(Key::Tab, Modifiers::NONE)]), Some(Ok("Tab".into())));
        assert_eq!(token(&[key(Key::ArrowLeft, Modifiers::NONE)]), Some(Ok("←".into())));
        assert_eq!(token(&[key(Key::Escape, Modifiers::NONE)]), Some(Err(())));
        assert_eq!(token(&[key(Key::Space, Modifiers::NONE), Event::Text(" ".into())]), Some(Ok("Space".into())));
    }

    #[test]
    fn sem_texto_deduz_pela_tecla() {
        assert_eq!(token(&[key(Key::X, Modifiers::NONE)]), Some(Ok("x".into())));
        assert_eq!(token(&[key(Key::C, Modifiers::SHIFT)]), Some(Ok("C".into())));
        assert_eq!(token(&[key(Key::Num3, Modifiers::NONE)]), Some(Ok("3".into())));
    }
}
