//! Som (o gemido do AIC) e notificação do sistema. Tudo fora da thread da UI.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// Toca um arquivo de áudio com o primeiro player disponível (PipeWire, Pulse, ffplay, ALSA).
pub fn play(path: &Path, volume: f32) {
    let path = path.to_path_buf();
    std::thread::spawn(move || {
        let v = volume.clamp(0.0, 1.5);
        let attempts: Vec<(&str, Vec<String>)> = vec![
            ("pw-play", vec!["--volume".into(), format!("{v:.2}"), path_str(&path)]),
            ("paplay", vec![format!("--volume={}", (v * 65536.0) as u32), path_str(&path)]),
            ("ffplay", vec!["-nodisp".into(), "-autoexit".into(), "-loglevel".into(), "quiet".into(), "-volume".into(), ((v * 100.0) as u32).to_string(), path_str(&path)]),
            ("aplay", vec!["-q".into(), path_str(&path)]),
        ];
        for (bin, args) in attempts {
            let ok = Command::new(bin)
                .args(&args)
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .is_ok_and(|s| s.success());
            if ok {
                return;
            }
        }
    });
}

fn path_str(p: &Path) -> String {
    p.to_string_lossy().to_string()
}

/// Notificação do desktop (notify-send via D-Bus).
pub fn desktop(title: &str, body: &str, icon: Option<PathBuf>) {
    let (title, body) = (title.to_string(), body.to_string());
    std::thread::spawn(move || {
        let mut n = notify_rust::Notification::new();
        n.appname("AIC").summary(&title).body(&body).timeout(6000);
        if let Some(i) = icon.filter(|p| p.exists()) {
            n.icon(&i.to_string_lossy());
        } else {
            n.icon("utilities-terminal");
        }
        let _ = n.show();
    });
}
