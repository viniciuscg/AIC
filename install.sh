#!/usr/bin/env bash
# Instala o AIC para o usuário atual (sem sudo). Pode rodar quantas vezes quiser.
#
#   ./install.sh              instala/atualiza
#   ./install.sh --dry-run    mostra o que faria, sem mudar nada
#   ./install.sh --uninstall  remove o que o install criou (dados e config ficam)
#
# O que faz: rustup (se faltar cargo), build release, link em ~/.local/bin/aic,
# ícones + atalho no menu, link do plugin do Claude e a fonte
# JetBrainsMono Nerd Font Mono (se faltar).

set -euo pipefail

REPO_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BIN_DIR="$HOME/.local/bin"
DATA_DIR="${XDG_DATA_HOME:-$HOME/.local/share}"
AIC_DATA="$DATA_DIR/aic"
ICON_DIR="$DATA_DIR/icons/hicolor"
DESKTOP_FILE="$DATA_DIR/applications/aic.desktop"
FONT_DIR="$DATA_DIR/fonts/JetBrainsMonoNerdFontMono"
ICON_SIZES=(16 32 48 64 128 256 512)

# Sons baixados por versões antigas (o uninstall ainda limpa).
OLD_SOUNDS=(plankton-gemido.mp3)
FONT_URL="https://github.com/ryanoasis/nerd-fonts/releases/latest/download/JetBrainsMono.tar.xz"
UA="Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0 Safari/537.36"

DRY=0
MODE=install
for arg in "$@"; do
  case "$arg" in
  --dry-run | -n) DRY=1 ;;
  --uninstall) MODE=uninstall ;;
  -h | --help)
    sed -n '2,9p' "$0" | sed 's/^# \{0,1\}//'
    exit 0
    ;;
  *)
    echo "opção desconhecida: $arg (use --help)" >&2
    exit 1
    ;;
  esac
done

if [ -t 1 ]; then
  G=$'\033[32m' Y=$'\033[33m' B=$'\033[34m' D=$'\033[2m' R=$'\033[31m' N=$'\033[0m'
else
  G='' Y='' B='' D='' R='' N=''
fi

STEP=0
TOTAL=0
step() {
  STEP=$((STEP + 1))
  printf '%s[%d/%d]%s %s ' "$B" "$STEP" "$TOTAL" "$N" "$1"
}
ok() { printf '%sok%s%s\n' "$G" "$N" "${1:+ $D($1)$N}"; }
done_() { printf '%sfeito%s%s\n' "$G" "$N" "${1:+ $D($1)$N}"; }
skip() { printf '%spulado%s%s\n' "$Y" "$N" "${1:+ $D($1)$N}"; }
warn() { printf '  %s!%s %s\n' "$Y" "$N" "$1"; }
fail() { printf '%serro%s %s\n' "$R" "$N" "$1"; }

# Executa (ou só mostra, no --dry-run).
run() {
  if [ "$DRY" = 1 ]; then
    printf '\n  %s$ %s%s' "$D" "$*" "$N"
  else
    "$@"
  fi
}
would() { [ "$DRY" = 1 ] && echo || true; }

have() { command -v "$1" >/dev/null 2>&1; }

# Link idempotente: ok se já aponta pro lugar certo.
link() {
  local target=$1 dest=$2
  if [ -L "$dest" ] && [ "$(readlink "$dest")" = "$target" ]; then
    ok
    return
  fi
  if [ -e "$dest" ] && [ ! -L "$dest" ]; then
    skip "$dest existe e não é link; remova à mão"
    return
  fi
  run mkdir -p "$(dirname "$dest")"
  run ln -sfn "$target" "$dest"
  [ "$DRY" = 1 ] && would || done_
}

fetch() {
  local url=$1 out=$2
  if have curl; then
    curl -fsSL --retry 2 -A "$UA" -o "$out" "$url"
  elif have wget; then
    wget -q -U "$UA" -O "$out" "$url"
  else
    return 1
  fi
}

cargo_bin() {
  if have cargo; then
    command -v cargo
  elif [ -x "$HOME/.cargo/bin/cargo" ]; then
    echo "$HOME/.cargo/bin/cargo"
  fi
}

# ---------------------------------------------------------------- instalar

install_rust() {
  step "Rust (cargo)"
  if [ -n "$(cargo_bin)" ]; then
    ok "$("$(cargo_bin)" --version 2>/dev/null)"
    return
  fi
  if ! have curl; then
    fail "precisa de curl para instalar o rustup"
    warn "instale: sudo apt install curl   (ou o equivalente da sua distro)"
    exit 1
  fi
  if [ "$DRY" = 1 ]; then
    run "curl https://sh.rustup.rs | sh -s -- -y --no-modify-path"
    would
    return
  fi
  curl --proto '=https' --tlsv1.2 -fsSL https://sh.rustup.rs | sh -s -- -y --no-modify-path --profile minimal >/dev/null
  done_ "rustup em ~/.cargo (sem mexer no PATH do shell)"
}

check_build_deps() {
  step "Compilador C (para o SQLite embutido)"
  if have cc || have gcc || have clang; then
    ok
  else
    skip "nenhum compilador C encontrado; o build vai falhar"
    warn "Debian/Ubuntu: sudo apt install build-essential"
    warn "Arch: sudo pacman -S base-devel   |   Fedora: sudo dnf install gcc"
  fi
}

build() {
  step "Compilando (cargo build --release)"
  local cargo
  cargo=$(cargo_bin)
  if [ "$DRY" = 1 ]; then
    run "${cargo:-cargo}" build --release --locked
    would
    return
  fi
  local bin="$REPO_DIR/target/release/aic"
  # Sem mudança no código desde o último build: o cargo resolve rápido, mas evita o barulho.
  if [ -x "$bin" ] && [ -z "$(find "$REPO_DIR/crates" "$REPO_DIR/docs/MANUAL.md" "$REPO_DIR/Cargo.toml" "$REPO_DIR/Cargo.lock" -newer "$bin" -print -quit 2>/dev/null)" ]; then
    ok "binário em dia"
    return
  fi
  local log
  log=$(mktemp)
  if (cd "$REPO_DIR" && "$cargo" build --release --locked) >"$log" 2>&1; then
    done_
    rm -f "$log"
  else
    fail "o build falhou; últimas linhas:"
    tail -n 20 "$log" | sed 's/^/    /'
    rm -f "$log"
    exit 1
  fi
}

install_bin() {
  step "Binário em ~/.local/bin/aic"
  link "$REPO_DIR/target/release/aic" "$BIN_DIR/aic"
  case ":$PATH:" in
  *":$BIN_DIR:"*) ;;
  *) warn "~/.local/bin não está no PATH; adicione ao seu shell: export PATH=\"\$HOME/.local/bin:\$PATH\"" ;;
  esac
}

install_icons() {
  step "Ícones (hicolor)"
  local bin="$REPO_DIR/target/release/aic"
  local all=1 s
  for s in "${ICON_SIZES[@]}"; do
    [ -f "$ICON_DIR/${s}x${s}/apps/aic.png" ] || all=0
  done
  if [ "$all" = 1 ] && [ -z "$(find "$bin" -newer "$ICON_DIR/512x512/apps/aic.png" -print -quit 2>/dev/null)" ]; then
    ok
    return
  fi
  if [ "$DRY" = 1 ]; then
    run "$bin" export-icon "<tmp>"
    would
    return
  fi
  local tmp
  tmp=$(mktemp -d)
  "$bin" export-icon "$tmp" >/dev/null
  for s in "${ICON_SIZES[@]}"; do
    mkdir -p "$ICON_DIR/${s}x${s}/apps"
    cp -f "$tmp/aic-$s.png" "$ICON_DIR/${s}x${s}/apps/aic.png"
  done
  rm -rf "$tmp"
  have gtk-update-icon-cache && gtk-update-icon-cache -q -t "$ICON_DIR" 2>/dev/null || true
  done_
}

desktop_entry() {
  cat <<EOF
[Desktop Entry]
Type=Application
Name=AIC
GenericName=Central de sessões do Claude Code
Comment=Sessões do Claude Code em blocos, com tmux por baixo
Exec=$BIN_DIR/aic
Icon=aic
Terminal=false
Categories=Development;TerminalEmulator;
Keywords=claude;tmux;terminal;
StartupWMClass=aic
EOF
}

install_desktop() {
  step "Atalho no menu (aic.desktop)"
  if [ -f "$DESKTOP_FILE" ] && [ "$(cat "$DESKTOP_FILE")" = "$(desktop_entry)" ]; then
    ok
    return
  fi
  if [ "$DRY" = 1 ]; then
    run "write $DESKTOP_FILE"
    would
    return
  fi
  mkdir -p "$(dirname "$DESKTOP_FILE")"
  desktop_entry >"$DESKTOP_FILE"
  have update-desktop-database && update-desktop-database -q "$(dirname "$DESKTOP_FILE")" 2>/dev/null || true
  done_
}

install_plugin() {
  step "Plugin do Claude (~/.local/share/aic/plugin)"
  link "$REPO_DIR/claude" "$AIC_DATA/plugin"
}

install_font() {
  step "Fonte JetBrainsMono Nerd Font Mono"
  if [ -n "$(find "$FONT_DIR" -name 'JetBrainsMonoNerdFontMono-*.ttf' -print -quit 2>/dev/null)" ] ||
    { have fc-list && fc-list 2>/dev/null | grep -qi "JetBrainsMono Nerd Font Mono"; }; then
    ok
    return
  fi
  if ! have tar || ! have xz; then
    skip "precisa de tar e xz para extrair a fonte"
    warn "ou baixe manualmente: https://www.nerdfonts.com/font-downloads"
    return
  fi
  if [ "$DRY" = 1 ]; then
    run fetch "$FONT_URL" "<tmp>" "&& tar -xJf ... 'JetBrainsMonoNerdFontMono-*.ttf' -C $FONT_DIR"
    would
    return
  fi
  local tmp
  tmp=$(mktemp -d)
  if fetch "$FONT_URL" "$tmp/font.tar.xz"; then
    mkdir -p "$FONT_DIR"
    tar -xJf "$tmp/font.tar.xz" -C "$FONT_DIR" --wildcards 'JetBrainsMonoNerdFontMono-*.ttf'
    have fc-cache && fc-cache -f "$FONT_DIR" >/dev/null 2>&1 || true
    done_ "$(find "$FONT_DIR" -name '*.ttf' | wc -l) arquivos em $FONT_DIR"
  else
    skip "download falhou; baixe em https://www.nerdfonts.com/font-downloads"
  fi
  rm -rf "$tmp"
}

check_runtime() {
  step "tmux e Claude Code"
  local missing=()
  have tmux || missing+=(tmux)
  have claude || [ -x "$HOME/.local/bin/claude" ] || missing+=(claude)
  if [ ${#missing[@]} -eq 0 ]; then
    ok
    return
  fi
  skip "faltando: ${missing[*]}"
  for m in "${missing[@]}"; do
    case $m in
    tmux) warn "tmux: sudo apt install tmux | sudo pacman -S tmux | sudo dnf install tmux" ;;
    claude) warn "Claude Code: curl -fsSL https://claude.ai/install.sh | bash" ;;
    esac
  done
}

# ---------------------------------------------------------------- remover

remove_path() {
  local p=$1
  if [ ! -e "$p" ] && [ ! -L "$p" ]; then
    skip "não existe"
    return
  fi
  run rm -rf "$p"
  [ "$DRY" = 1 ] && would || done_
}

uninstall() {
  TOTAL=6
  step "Binário ~/.local/bin/aic"
  if [ -L "$BIN_DIR/aic" ]; then remove_path "$BIN_DIR/aic"; else skip "não é link do AIC"; fi
  step "Atalho no menu"
  remove_path "$DESKTOP_FILE"
  step "Ícones"
  local s found=0
  for s in "${ICON_SIZES[@]}"; do
    [ -f "$ICON_DIR/${s}x${s}/apps/aic.png" ] && found=1 && run rm -f "$ICON_DIR/${s}x${s}/apps/aic.png"
  done
  if [ "$found" = 1 ]; then [ "$DRY" = 1 ] && would || done_; else skip "não existem"; fi
  step "Link do plugin"
  if [ -L "$AIC_DATA/plugin" ]; then remove_path "$AIC_DATA/plugin"; else skip "não é link"; fi
  step "Sons baixados (versões antigas)"
  local f found=0
  for f in "${OLD_SOUNDS[@]}"; do
    [ -f "$AIC_DATA/sounds/$f" ] && found=1 && run rm -f "$AIC_DATA/sounds/$f"
  done
  if [ "$found" = 1 ]; then [ "$DRY" = 1 ] && would || done_; else skip "não existem"; fi
  step "Fonte instalada pelo AIC"
  remove_path "$FONT_DIR"
  echo
  echo "Ficaram (apague à mão se quiser): ~/.config/aic, $AIC_DATA, ~/.local/state/aic, ~/.cargo"
}

# ---------------------------------------------------------------- main

if [ "$(id -u)" = 0 ]; then
  echo "Não rode como root: o AIC é instalado só para o seu usuário." >&2
  exit 1
fi

[ "$DRY" = 1 ] && echo "${D}--dry-run: nada será alterado${N}"

if [ "$MODE" = uninstall ]; then
  uninstall
  exit 0
fi

TOTAL=9
install_rust
check_build_deps
build
install_bin
install_icons
install_desktop
install_plugin
install_font
check_runtime

echo
echo "${G}Pronto.${N} Abra pelo menu ou rode: aic"
[ -f "$HOME/.config/aic/.env" ] || echo "${D}Integrações opcionais: copie .env.example para ~/.config/aic/.env${N}"
