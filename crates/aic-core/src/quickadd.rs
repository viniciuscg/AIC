//! Captura rápida: uma linha de texto vira tarefa.
//!
//! ```text
//! corrigir cálculo de juros #api !alta @hoje ^DEV-142 +sprint
//! ```
//!
//! | marcador        | vira                                   |
//! |-----------------|----------------------------------------|
//! | `#repo`         | repo (casado por prefixo com os repos) |
//! | `@hoje` `@sexta` `@12/10` | prazo                        |
//! | `!alta` `!!` `!!!` `!baixa` | prioridade                 |
//! | `+workspace`    | workspace (por prefixo do nome)        |
//! | `^DEV-142`     | chave do card (vira nome de branch)    |
//! | `~tag`          | etiqueta livre                         |
//!
//! Marcadores podem estar em qualquer posição; o resto é o título.

use chrono::{Datelike, Duration, NaiveDate, Weekday};

use crate::model::{Priority, Task};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChipKind {
    Repo,
    Due,
    Priority,
    Workspace,
    Card,
    Tag,
    /// Marcador que não foi reconhecido (fica no título e é sinalizado na UI).
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Chip {
    pub kind: ChipKind,
    pub label: String,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Parsed {
    pub title: String,
    pub repo: Option<String>,
    pub due: Option<NaiveDate>,
    pub priority: Option<Priority>,
    pub workspace: Option<String>,
    pub card_key: Option<String>,
    pub tags: Vec<String>,
    pub chips: Vec<Chip>,
}

impl Parsed {
    /// Monta a tarefa. `workspace_id` resolve o nome do `+workspace` (ou usa o atual).
    pub fn into_task(self) -> Task {
        let mut t = Task::new(self.title);
        t.repo = self.repo;
        t.due = self.due;
        t.priority = self.priority.unwrap_or_default();
        t.card_key = self.card_key;
        t.tags = self.tags;
        t
    }
}

/// `repos` e `workspaces` são os nomes conhecidos, pra casar `#api` → `api-pagamentos`.
pub fn parse(input: &str, today: NaiveDate, repos: &[String], workspaces: &[String]) -> Parsed {
    let mut p = Parsed::default();
    let mut title_words: Vec<&str> = Vec::new();

    for word in input.split_whitespace() {
        let (sigil, rest) = word.split_at(word.chars().next().map(|c| c.len_utf8()).unwrap_or(0));
        let handled = match sigil {
            "#" if !rest.is_empty() => {
                let repo = match_name(rest, repos).unwrap_or_else(|| rest.to_string());
                p.chips.push(Chip { kind: ChipKind::Repo, label: repo.clone() });
                p.repo = Some(repo);
                true
            }
            "@" if !rest.is_empty() => match parse_date(rest, today) {
                Some(d) => {
                    p.chips.push(Chip { kind: ChipKind::Due, label: due_label(d, today) });
                    p.due = Some(d);
                    true
                }
                None => {
                    p.chips.push(Chip { kind: ChipKind::Unknown, label: word.to_string() });
                    false
                }
            },
            "!" => match parse_priority(rest) {
                Some(pr) => {
                    p.chips.push(Chip { kind: ChipKind::Priority, label: pr.label().to_string() });
                    p.priority = Some(pr);
                    true
                }
                None => false,
            },
            "+" if !rest.is_empty() => {
                let ws = match_name(rest, workspaces).unwrap_or_else(|| rest.to_string());
                p.chips.push(Chip { kind: ChipKind::Workspace, label: ws.clone() });
                p.workspace = Some(ws);
                true
            }
            "^" if !rest.is_empty() => {
                let key = rest.to_uppercase();
                p.chips.push(Chip { kind: ChipKind::Card, label: key.clone() });
                p.card_key = Some(key);
                true
            }
            "~" if !rest.is_empty() => {
                p.chips.push(Chip { kind: ChipKind::Tag, label: rest.to_string() });
                p.tags.push(rest.to_string());
                true
            }
            _ => false,
        };
        if !handled {
            title_words.push(word);
        }
    }

    p.title = title_words.join(" ");
    p
}

/// Casa por igualdade, depois prefixo, depois "contém" — ignorando caixa.
fn match_name(q: &str, names: &[String]) -> Option<String> {
    let q = q.to_lowercase();
    let lower: Vec<String> = names.iter().map(|n| n.to_lowercase()).collect();
    lower
        .iter()
        .position(|n| *n == q)
        .or_else(|| lower.iter().position(|n| n.starts_with(&q)))
        .or_else(|| lower.iter().position(|n| n.contains(&q)))
        .map(|i| names[i].clone())
}

fn parse_priority(s: &str) -> Option<Priority> {
    Some(match s.to_lowercase().as_str() {
        "" | "a" | "alta" | "high" | "h" => Priority::High,
        "!" | "!!" | "u" | "urgente" | "urgent" => Priority::Urgent,
        "b" | "baixa" | "low" | "l" => Priority::Low,
        "n" | "normal" => Priority::Normal,
        _ => return None,
    })
}

fn weekday(s: &str) -> Option<Weekday> {
    let s = strip_accents(s);
    Some(match s.as_str() {
        "seg" | "segunda" => Weekday::Mon,
        "ter" | "terca" => Weekday::Tue,
        "qua" | "quarta" => Weekday::Wed,
        "qui" | "quinta" => Weekday::Thu,
        "sex" | "sexta" => Weekday::Fri,
        "sab" | "sabado" => Weekday::Sat,
        "dom" | "domingo" => Weekday::Sun,
        _ => return None,
    })
}

fn strip_accents(s: &str) -> String {
    s.to_lowercase()
        .chars()
        .map(|c| match c {
            'á' | 'à' | 'â' | 'ã' => 'a',
            'é' | 'ê' => 'e',
            'í' => 'i',
            'ó' | 'ô' | 'õ' => 'o',
            'ú' => 'u',
            'ç' => 'c',
            c => c,
        })
        .collect()
}

pub fn parse_date(s: &str, today: NaiveDate) -> Option<NaiveDate> {
    let k = strip_accents(s);
    match k.as_str() {
        "hoje" | "hj" => return Some(today),
        "amanha" | "amn" => return Some(today + Duration::days(1)),
        "depois" => return Some(today + Duration::days(2)),
        // fim da semana útil
        "semana" => return Some(next_weekday(today, Weekday::Fri, true)),
        "proxsemana" | "proxima" => return Some(next_weekday(today, Weekday::Mon, false)),
        _ => {}
    }
    if let Some(wd) = weekday(&k) {
        return Some(next_weekday(today, wd, false));
    }
    // +3 / 3d = daqui a 3 dias
    if let Some(n) = k.strip_suffix('d').or_else(|| k.strip_prefix('+')).and_then(|n| n.parse::<i64>().ok()) {
        return Some(today + Duration::days(n));
    }
    if let Ok(d) = NaiveDate::parse_from_str(&k, "%Y-%m-%d") {
        return Some(d);
    }
    let parts: Vec<&str> = k.split('/').collect();
    match parts.as_slice() {
        [d, m] => {
            let (d, m) = (d.parse().ok()?, m.parse().ok()?);
            let this_year = NaiveDate::from_ymd_opt(today.year(), m, d)?;
            // 05/01 em dezembro = janeiro do ano que vem
            Some(if this_year < today { NaiveDate::from_ymd_opt(today.year() + 1, m, d)? } else { this_year })
        }
        [d, m, y] => {
            let mut y: i32 = y.parse().ok()?;
            if y < 100 {
                y += 2000;
            }
            NaiveDate::from_ymd_opt(y, m.parse().ok()?, d.parse().ok()?)
        }
        _ => None,
    }
}

/// Próximo `wd` a partir de amanhã (ou hoje, se `include_today`).
fn next_weekday(today: NaiveDate, wd: Weekday, include_today: bool) -> NaiveDate {
    let start = if include_today { 0 } else { 1 };
    (start..=7)
        .map(|i| today + Duration::days(i))
        .find(|d| d.weekday() == wd)
        .unwrap_or(today)
}

pub fn due_label(d: NaiveDate, today: NaiveDate) -> String {
    let diff = (d - today).num_days();
    match diff {
        0 => "hoje".into(),
        1 => "amanhã".into(),
        -1 => "ontem".into(),
        2..=6 => {
            let wd = ["seg", "ter", "qua", "qui", "sex", "sáb", "dom"][d.weekday().num_days_from_monday() as usize];
            wd.to_string()
        }
        n if n < 0 => format!("{} dias atrás", -n),
        _ => d.format("%d/%m").to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn today() -> NaiveDate {
        // quinta-feira
        NaiveDate::from_ymd_opt(2026, 10, 8).unwrap()
    }

    fn repos() -> Vec<String> {
        ["api-pagamentos", "api-core", "web-canvas", "infra"].map(String::from).to_vec()
    }

    #[test]
    fn exemplo_completo() {
        let p = parse("corrigir cálculo de juros #api !alta @hoje ^dev-142", today(), &repos(), &[]);
        assert_eq!(p.title, "corrigir cálculo de juros");
        assert_eq!(p.repo.as_deref(), Some("api-pagamentos"));
        assert_eq!(p.priority, Some(Priority::High));
        assert_eq!(p.due, Some(today()));
        assert_eq!(p.card_key.as_deref(), Some("DEV-142"));
        assert_eq!(p.chips.len(), 4);
    }

    #[test]
    fn so_texto_vira_titulo() {
        let p = parse("revisar PR do notificador", today(), &repos(), &[]);
        assert_eq!(p.title, "revisar PR do notificador");
        assert!(p.chips.is_empty());
    }

    #[test]
    fn marcadores_em_qualquer_posicao() {
        let p = parse("!! #web-c ajustar canvas @sexta ~front", today(), &repos(), &[]);
        assert_eq!(p.title, "ajustar canvas");
        assert_eq!(p.priority, Some(Priority::Urgent));
        assert_eq!(p.repo.as_deref(), Some("web-canvas"));
        assert_eq!(p.due, NaiveDate::from_ymd_opt(2026, 10, 9));
        assert_eq!(p.tags, vec!["front"]);
    }

    #[test]
    fn datas() {
        let t = today();
        assert_eq!(parse_date("amanhã", t), NaiveDate::from_ymd_opt(2026, 10, 9));
        assert_eq!(parse_date("segunda", t), NaiveDate::from_ymd_opt(2026, 10, 12));
        assert_eq!(parse_date("qui", t), NaiveDate::from_ymd_opt(2026, 10, 15)); // próxima, não hoje
        assert_eq!(parse_date("semana", t), NaiveDate::from_ymd_opt(2026, 10, 9));
        assert_eq!(parse_date("3d", t), NaiveDate::from_ymd_opt(2026, 10, 11));
        assert_eq!(parse_date("20/10", t), NaiveDate::from_ymd_opt(2026, 10, 20));
        assert_eq!(parse_date("05/01", t), NaiveDate::from_ymd_opt(2027, 1, 5));
        assert_eq!(parse_date("2026-12-24", t), NaiveDate::from_ymd_opt(2026, 12, 24));
        assert_eq!(parse_date("xyz", t), None);
    }

    #[test]
    fn exclamacao_solta_no_titulo_nao_some() {
        let p = parse("deploy urgente!", today(), &repos(), &[]);
        assert_eq!(p.title, "deploy urgente!");
    }

    #[test]
    fn data_invalida_fica_no_titulo() {
        let p = parse("falar com @fulano", today(), &repos(), &[]);
        assert_eq!(p.title, "falar com @fulano");
        assert_eq!(p.chips[0].kind, ChipKind::Unknown);
    }

    #[test]
    fn workspace_por_prefixo() {
        let ws = vec!["Sprint 42".to_string(), "Infra".to_string()];
        let p = parse("x +spr", today(), &repos(), &ws);
        assert_eq!(p.workspace.as_deref(), Some("Sprint 42"));
    }
}
