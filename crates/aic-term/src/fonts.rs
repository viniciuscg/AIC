//! Fontes via fontconfig: Nerd Font principal + cadeia de fallback para os
//! símbolos que o Claude Code usa (✻ ⎿ ⏵), CJK e emoji.

use std::process::Command;
use std::sync::Arc;

use egui::{FontData, FontDefinitions, FontFamily, FontId};

use crate::view::TerminalFont;

/// Família que o AIC registra para o negrito do terminal.
pub const BOLD_FAMILY: &str = "aic-mono-bold";

/// Fallbacks na ordem em que são tentados depois da fonte principal.
const FALLBACKS: &[&str] = &[
    "DejaVu Sans Mono",
    "Noto Sans Symbols 2",
    "Noto Sans Symbols",
    "Noto Sans Math",
    "Noto Sans Mono CJK SC",
    "Noto Color Emoji",
];

/// Resolve `família[:estilo]` para (arquivo, índice dentro do .ttc). Só aceita se o
/// fontconfig devolver a família pedida, e não um substituto qualquer.
pub fn find_font(pattern: &str) -> Option<(String, u32)> {
    let out = Command::new("fc-match")
        .args(["-f", "%{family}\t%{file}\t%{index}", pattern])
        .output()
        .ok()?;
    let line = String::from_utf8_lossy(&out.stdout).to_string();
    let mut parts = line.split('\t');
    let families = parts.next()?;
    let file = parts.next()?.to_string();
    let index = parts
        .next()
        .and_then(|i| i.trim().parse().ok())
        .unwrap_or(0);
    let wanted = pattern.split(':').next()?.to_lowercase();
    let ok = families.split(',').any(|f| {
        let f = f.to_lowercase();
        f == wanted || f.replace(' ', "") == wanted.replace(' ', "")
    });
    ok.then_some((file, index))
}

fn load(pattern: &str) -> Option<FontData> {
    let (file, index) = find_font(pattern)?;
    let bytes = std::fs::read(file).ok()?;
    let mut data = FontData::from_owned(bytes);
    data.index = index;
    Some(data)
}

/// Monta as fontes do app. `mono` é a família do terminal (ex: "JetBrainsMono Nerd Font Mono");
/// `ui` (opcional) é a fonte proporcional da interface. Devolve a fonte pronta pro terminal.
pub fn install(ctx: &egui::Context, mono: &str, ui: Option<&str>, size: f32) -> TerminalFont {
    let mut defs = FontDefinitions::default();
    let mut mono_chain: Vec<String> = Vec::new();

    let add = |defs: &mut FontDefinitions, name: &str, data: FontData| {
        defs.font_data.insert(name.to_string(), Arc::new(data));
    };

    if let Some(d) = load(mono) {
        add(&mut defs, "mono", d);
        mono_chain.push("mono".into());
    }
    let mut bold_chain = Vec::new();
    if let Some(d) = load(&format!("{mono}:bold")) {
        add(&mut defs, "mono-bold", d);
        bold_chain.push("mono-bold".to_string());
    }
    for (i, fb) in FALLBACKS.iter().enumerate() {
        if let Some(d) = load(fb) {
            let name = format!("fallback-{i}");
            add(&mut defs, &name, d);
            mono_chain.push(name);
        }
    }

    // Monospace = principal + fallbacks + padrão do egui no fim.
    let defaults = defs
        .families
        .get(&FontFamily::Monospace)
        .cloned()
        .unwrap_or_default();
    let mut chain = mono_chain.clone();
    chain.extend(defaults);
    defs.families.insert(FontFamily::Monospace, chain.clone());

    let has_bold = !bold_chain.is_empty();
    if has_bold {
        bold_chain.extend(chain.iter().skip(1).cloned());
        defs.families
            .insert(FontFamily::Name(BOLD_FAMILY.into()), bold_chain);
    }

    // Interface: fonte escolhida (se houver) + os mesmos fallbacks, pra ícones Nerd na UI.
    let prop = defs.families.entry(FontFamily::Proportional).or_default();
    if let Some(name) = ui {
        if let Some(d) = load(name) {
            defs.font_data.insert("ui".into(), Arc::new(d));
            prop.insert(0, "ui".into());
        }
    }
    let prop = defs.families.get_mut(&FontFamily::Proportional).unwrap();
    for name in &mono_chain {
        if !prop.contains(name) {
            prop.push(name.clone());
        }
    }

    ctx.set_fonts(defs);
    TerminalFont {
        regular: FontId::monospace(size),
        bold: has_bold.then(|| FontId::new(size, FontFamily::Name(BOLD_FAMILY.into()))),
    }
}
