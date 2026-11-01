# egui-winit 0.36.2 (vendorizado)

Cópia do crate do crates.io com uma mudança em `src/lib.rs` (procure `AIC:`):

- Ctrl+V sem texto no clipboard (só imagem) não some mais: vira `Event::Key` normal.
  O terminal do AIC manda o byte do Ctrl+V e o Claude Code lê a imagem do clipboard.
  Sem isso o egui engolia a tecla e colar imagem não funcionava.

Ao subir a versão do egui, copie o crate novo pra cá e reaplique o ajuste.
