//! Medição de custo por frame, no cenário real: dois terminais cheios a 2000×1100 com fonte 8,
//! mais 10 sessões escondidas soltando saída. Roda o que o eframe roda (`run_ui` + `tessellate`)
//! e respeita os pedidos de redesenho, como o loop de verdade.
//!
//!   cargo test --release -p aic-app --test perf -- --ignored --nocapture --test-threads=1

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use aic_app::app::App;
use aic_core::SessionKind;
use egui::{Context, Event, Pos2, RawInput, Rect, Vec2};

fn sandbox() {
    let dir = std::env::temp_dir().join(format!("aic-perf-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    for d in ["config", "data", "state", "claude/projects"] {
        std::fs::create_dir_all(dir.join(d)).unwrap();
    }
    // SAFETY: só este teste roda neste processo (--test-threads=1)
    unsafe {
        std::env::set_var("AIC_CONFIG_DIR", dir.join("config"));
        std::env::set_var("AIC_DATA_DIR", dir.join("data"));
        std::env::set_var("AIC_STATE_DIR", dir.join("state"));
        std::env::set_var("CLAUDE_CONFIG_DIR", dir.join("claude"));
    }
    std::fs::write(
        dir.join("config/config.toml"),
        "setup_done = true\nfont_size = 8.0\n\n[anakin]\nauto_start = false\n\n[modules]\ntmux = false\nsound = false\nnotifications = false\n",
    )
    .unwrap();
}

fn input() -> RawInput {
    RawInput {
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(2000.0, 1100.0))),
        focused: true,
        ..Default::default()
    }
}

struct Bench {
    ctx: Context,
    app: Option<App>,
    next: Arc<Mutex<Option<Instant>>>,
}

impl Bench {
    fn new() -> Self {
        sandbox();
        let ctx = Context::default();
        let next: Arc<Mutex<Option<Instant>>> = Arc::new(Mutex::new(None));
        let n = next.clone();
        ctx.set_request_repaint_callback(move |info| {
            let at = Instant::now() + info.delay;
            let mut g = n.lock().unwrap();
            *g = Some(g.map_or(at, |x| x.min(at)));
        });
        let mut b = Self {
            ctx,
            app: None,
            next,
        };
        b.frame(vec![]);
        b.frame(vec![]);
        b
    }

    fn frame(&mut self, events: Vec<Event>) -> usize {
        let ctx = self.ctx.clone();
        let mut raw = input();
        raw.events = events;
        let app = &mut self.app;
        let mut out = ctx.run_ui(raw, |ui| {
            let a = app.get_or_insert_with(|| App::new(ui.ctx(), true).expect("app"));
            aic_app::ui::draw(a, ui);
        });
        out.textures_delta.clear(); // sem GPU aqui: ninguém aplica as texturas
        let prims = ctx.tessellate(out.shapes, out.pixels_per_point);
        prims
            .iter()
            .map(|p| match &p.primitive {
                egui::epaint::Primitive::Mesh(m) => m.vertices.len(),
                _ => 0,
            })
            .sum()
    }

    fn app(&mut self) -> &mut App {
        self.app.as_mut().unwrap()
    }

    /// Terminal rodando `cmd` num bloco (novo ou o último criado).
    fn term(&mut self, name: &str, cmd: &str, new_block: bool) {
        let ctx = self.ctx.clone();
        let a = self.app();
        let block = if new_block {
            a.new_block(name)
        } else {
            a.current_block()
        };
        a.create_session_in(
            &ctx,
            block,
            name.into(),
            SessionKind::Command(cmd.into()),
            std::env::temp_dir(),
            None,
            vec![],
            None,
        );
    }

    fn settle(&mut self, ms: u64) {
        let until = Instant::now() + Duration::from_millis(ms);
        while Instant::now() < until {
            self.frame(vec![]);
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    /// CPU (utime+stime) do processo inteiro em segundos.
    fn cpu() -> f64 {
        let s = std::fs::read_to_string("/proc/self/stat").unwrap();
        let rest = s.rsplit(')').next().unwrap();
        let v: Vec<&str> = rest.split_whitespace().collect();
        (v[11].parse::<f64>().unwrap() + v[12].parse::<f64>().unwrap()) / 100.0
    }

    fn timed(&mut self, label: &str, n: usize) {
        let mut times = Vec::with_capacity(n);
        let mut verts = 0;
        for _ in 0..n {
            let t = Instant::now();
            verts = self.frame(vec![]);
            times.push(t.elapsed());
        }
        times.sort();
        println!(
            "{label:<34} p50 {:>8.1?}  p95 {:>8.1?}  ({verts} vértices)",
            times[n / 2],
            times[n * 95 / 100],
        );
    }

    /// Roda como o loop real: só desenha quando alguém pediu. Quantos frames e quanta CPU?
    fn real_loop(&mut self, label: &str, secs: u64) {
        let start_cpu = Self::cpu();
        let start = Instant::now();
        let mut frames = 0u32;
        *self.next.lock().unwrap() = Some(Instant::now());
        while start.elapsed() < Duration::from_secs(secs) {
            let due = {
                let mut g = self.next.lock().unwrap();
                match *g {
                    Some(at) if at <= Instant::now() => {
                        *g = None;
                        true
                    }
                    _ => false,
                }
            };
            if due {
                self.frame(vec![]);
                frames += 1;
            } else {
                std::thread::sleep(Duration::from_millis(1));
            }
        }
        let cpu = Self::cpu() - start_cpu;
        println!(
            "{label:<34} {:>6.1} frames/s   {:>6.1}% de um núcleo",
            frames as f64 / secs as f64,
            cpu / secs as f64 * 100.0
        );
    }
}

const FILL: &str = r#"for i in $(seq 1 300); do printf '\033[3%dm● linha %d\033[0m lorem ipsum dolor sit amet \033[1mconsectetur\033[0m adipiscing elit, sed do eiusmod \033[44m tempor \033[0m incididunt ut labore et dolore magna aliqua %s\n' $((i%7+1)) $i "$(printf 'x%.0s' $(seq 1 $((i%60))))"; done; sleep 100000"#;

#[test]
#[ignore]
fn custo_por_frame() {
    let mut b = Bench::new();
    b.term("cheio", FILL, true);
    b.term("cheio2", FILL, false);
    b.settle(1500);
    b.timed("ocioso, 2 terminais cheios", 300);

    // um "spinner" num terminal: uma linha muda, o resto fica igual (o Claude trabalhando)
    b.term(
        "spinner",
        r#"i=0; while :; do i=$((i+1)); printf '\r\033[33m✻ Trabalhando… %d\033[0m' $i; sleep 0.03; done"#,
        false,
    );
    b.settle(500);
    b.timed("1 linha mudando, 3 terminais", 300);

    // o teclado: digitando no terminal em foco
    let t = Instant::now();
    for _ in 0..300 {
        b.frame(vec![Event::Text("x".into())]);
    }
    println!(
        "{:<34} {:>8.1?} por tecla",
        "frame com tecla",
        t.elapsed() / 300
    );
    let a = b.app();
    let ids: Vec<_> = a.sessions.keys().copied().collect();
    for i in ids {
        a.kill_session(i, false);
    }
}

#[test]
#[ignore]
fn sessoes_escondidas() {
    let mut b = Bench::new();
    b.term("visivel", FILL, true);
    b.settle(3000); // o script acaba de imprimir antes de medir o "parado"
    b.real_loop("nada acontecendo", 5);
    let visible = b.app().current_block();
    // 10 Claudes trabalhando em outras sessões, soltando saída o tempo todo
    for i in 0..10 {
        b.term(
            &format!("h{i}"),
            r#"i=0; while :; do i=$((i+1)); echo "✻ linha $i de saída de quem está escondido"; sleep 0.05; done"#,
            true,
        );
    }
    b.app().focus_block(visible.unwrap());
    b.settle(500);
    b.real_loop("10 sessões escondidas com saída", 5);
    let a = b.app();
    let ids: Vec<_> = a.sessions.keys().copied().collect();
    for i in ids {
        a.kill_session(i, false);
    }
}
