//! Uma linha de texto vira tarefa: `pagar boleto @sexta`. O `@data` é opcional e pode estar em
//! qualquer posição; o resto é o título. `@fulano` que não é data fica no título.

use chrono::{Datelike, Duration, NaiveDate, Weekday};

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Parsed {
    pub title: String,
    pub due: Option<NaiveDate>,
}

pub fn parse(input: &str, today: NaiveDate) -> Parsed {
    let mut p = Parsed::default();
    let mut words: Vec<&str> = Vec::new();
    for word in input.split_whitespace() {
        match word.strip_prefix('@').and_then(|d| parse_date(d, today)) {
            Some(d) => p.due = Some(d),
            None => words.push(word),
        }
    }
    p.title = words.join(" ");
    p
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
    if let Some(n) = k
        .strip_suffix('d')
        .or_else(|| k.strip_prefix('+'))
        .and_then(|n| n.parse::<i64>().ok())
    {
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
            Some(if this_year < today {
                NaiveDate::from_ymd_opt(today.year() + 1, m, d)?
            } else {
                this_year
            })
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
            let wd = ["seg", "ter", "qua", "qui", "sex", "sáb", "dom"]
                [d.weekday().num_days_from_monday() as usize];
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

    #[test]
    fn data_em_qualquer_posicao() {
        let p = parse("@sexta pagar o boleto", today());
        assert_eq!(p.title, "pagar o boleto");
        assert_eq!(p.due, NaiveDate::from_ymd_opt(2026, 10, 9));
        assert_eq!(parse("só texto", today()).due, None);
    }

    #[test]
    fn arroba_que_nao_e_data_fica_no_titulo() {
        let p = parse("falar com @fulano", today());
        assert_eq!(p.title, "falar com @fulano");
        assert_eq!(p.due, None);
    }

    #[test]
    fn datas() {
        let t = today();
        assert_eq!(
            parse_date("amanhã", t),
            NaiveDate::from_ymd_opt(2026, 10, 9)
        );
        assert_eq!(
            parse_date("segunda", t),
            NaiveDate::from_ymd_opt(2026, 10, 12)
        );
        assert_eq!(parse_date("qui", t), NaiveDate::from_ymd_opt(2026, 10, 15)); // próxima, não hoje
        assert_eq!(
            parse_date("semana", t),
            NaiveDate::from_ymd_opt(2026, 10, 9)
        );
        assert_eq!(parse_date("3d", t), NaiveDate::from_ymd_opt(2026, 10, 11));
        assert_eq!(
            parse_date("20/10", t),
            NaiveDate::from_ymd_opt(2026, 10, 20)
        );
        assert_eq!(parse_date("05/01", t), NaiveDate::from_ymd_opt(2027, 1, 5));
        assert_eq!(
            parse_date("2026-12-24", t),
            NaiveDate::from_ymd_opt(2026, 12, 24)
        );
        assert_eq!(parse_date("xyz", t), None);
    }
}
