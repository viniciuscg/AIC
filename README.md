# AIC

> down in a hole, up in the stack

Uma central desktop para quem trabalha com vários [Claude Code](https://claude.com/claude-code) ao mesmo tempo. Substitui o tmux: cada contexto de trabalho (um card, um bug, uma ideia) vira um **bloco** com seus terminais, notas, tarefas e o card do Notion, e o AIC mostra de relance quem está trabalhando, quem terminou e quem precisa de você.

Feito em Rust com [egui](https://github.com/emilk/egui). Roda no Linux (X11 e Wayland).

![Blocos](docs/aic-blocos.png)

## O que faz

- **Blocos**: terminais (Claude ou shell), notas em Markdown, tarefas e card agrupados por contexto. Quatro visualizações: blocos, abas, foco e grade.
- **tmux por baixo**: os terminais rodam num servidor tmux próprio. Fechou a janela, o app caiu? Os Claudes continuam rodando e o AIC reconecta ao abrir.
- **Estado de cada sessão** pelos hooks do Claude Code: trabalhando, esperando você, terminou, morreu. Um atalho pula para a próxima que precisa de você; som e notificação avisam quando termina.
- **Prefixo estilo tmux** (`Ctrl+B`): `Ctrl+B c` abre um Claude, `Ctrl+B x` fecha o que está em foco, `Ctrl+B n/p` troca de aba, `Ctrl+B 1-9` troca de workspace. Funciona com o terminal focado.
- **Catálogo de atalhos** (`Ctrl+Space` ou `Ctrl+B ?`): todos os atalhos agrupados, com o atalho direto e o acorde do prefixo de cada um.
- **Barra no estilo i3**: workspaces numerados em destaque, abas com a tecla do prefixo e status (quem espera você, custo, limites, relógio) à direita.
- **Workspaces como contexto**: a tela mostra só o workspace aceso na barra; pra ver outro, troca de workspace.
- **Captura rápida**: uma linha vira tarefa, com marcadores (`#repo @hoje !alta ^DEV-142`).
- **Layne**: um assistente (Claude sempre aberto no painel lateral) que você usa por texto ou voz pra criar workspaces, anotar tarefas e abrir sessões e cards por você.
- **Abrir card**: cole o link de um card e o AIC monta o bloco com um worktree e um Claude por repositório.
- **Board** kanban com tarefas locais e cards do Notion, **worktrees** por card e uma aba de **tokens** com limites da conta e custo por sessão.
- **Módulos**: tudo além dos terminais liga e desliga.
- **Config com busca**: `Ctrl+B /` e digita (`fonte`, `som`, `notion`…).
- Visual inspirado no grunge de Seattle: mascote animado (um pote de moscas), nomes de música para os estados, granulação de filme.

![Prefixo armado](docs/aic-prefixo.png)

| | |
| --- | --- |
| ![Board](docs/aic-board.png) | ![Captura rápida](docs/aic-captura.png) |
| Board | Captura rápida |

![Tokens](docs/aic-tokens.png)

## Instalação

Precisa de `git`, um compilador C (`build-essential` no Debian/Ubuntu), `tmux` e o Claude Code. O resto o instalador resolve, sem sudo.

```bash
git clone https://github.com/viniciuscg/AIC.git ~/AIC
cd ~/AIC
./install.sh
```

O `install.sh` pode rodar quantas vezes quiser; cada passo diz `ok`, `feito` ou `pulado`:

1. instala o Rust com o rustup se não houver `cargo` (sem mexer no seu `PATH`);
2. compila (`cargo build --release`);
3. cria o link `~/.local/bin/aic`;
4. instala os ícones e o atalho no menu (`~/.local/share/applications/aic.desktop`);
5. liga o plugin do Claude: `~/.local/share/aic/plugin` → `claude/` do repo;
6. instala a fonte JetBrainsMono Nerd Font Mono se ela não estiver no sistema;
7. avisa se faltar `tmux` ou `claude`.

```bash
./install.sh --dry-run     # mostra o que faria
./install.sh --uninstall   # remove binário, atalho, ícones, link do plugin e fonte
```

Para atualizar: `git pull && ./install.sh`.

## Uso

```bash
aic                          # abre o app
aic --fresh                  # começa sem reabrir as sessões salvas
aic add "revisar PR #api !alta @amanha"   # cria tarefa pelo terminal
aic tasks [--all] [--json]   # lista tarefas
aic done <id>                # conclui
aic ws new 'Pessoal' --go    # workspaces, sessões e cards no app aberto (veja Layne)
aic voz                      # liga/desliga a gravação pro Layne
```

Na primeira vez, um assistente pergunta a visualização, os módulos, as pastas dos seus repositórios, o prefixo e a tecla do catálogo.

### Prefixo (o jeito tmux)

Aperte `Ctrl+B`, solte e aperte a tecla. A barra de cima acende enquanto o prefixo espera; `Esc` cancela e `Ctrl+B Ctrl+B` manda o próprio Ctrl+B pro terminal.

| Tecla | Ação |
| --- | --- |
| `c` / `%` | novo Claude / shell ao lado |
| `L` / `m` | painel do Layne / falar com ele |
| `x` | fecha o que está em foco (terminal, bloco vazio, cartão aberto ou a aba) |
| `h j k l`, setas | move o foco |
| `z` | maximiza o terminal |
| `a` | próxima sessão que precisa de você |
| `n` / `p` | próxima / anterior aba (Sessões → Board → Worktrees → Tokens → Config → Ajuda) |
| `s` `b` `g` `u` `,` | vai direto pra aba |
| `1`-`9`, `(` `)`, `Tab` | workspace N, anterior/próximo, voltar pro de antes |
| `w` `W` `$` | lista, novo e renomear workspace |
| `/` | pesquisar nas configurações |
| `?` | catálogo com todos os atalhos |

O prefixo e cada tecla mudam em Config → Atalhos ou na tabela `[prefix]` do `keybinds.toml`. Os atalhos diretos (`Ctrl+Shift+…`) continuam valendo.

### Blocos e visualizações

`Ctrl+B B` (ou `Ctrl+Shift+Enter`) cria um bloco, `Ctrl+B c` abre um Claude nele (escolhendo pasta ou worktree), `Ctrl+B %` abre um shell ao lado. `Ctrl+B v` troca a visualização:

| Visualização | Como fica |
| --- | --- |
| Blocos | cada bloco com seus terminais, um embaixo do outro |
| Abas | um bloco por aba, terminais divididos; o jeito tmux |
| Foco | um terminal grande e a lista de sessões ao lado |
| Grade | todos os terminais do workspace numa grade |

`Alt+1..9` pula para o bloco N; `Ctrl+B :` abre a paleta de comandos (ações, sessões do workspace e tarefas).

### Captura rápida

`Ctrl+Shift+Space` abre a captura rápida. Os marcadores podem estar em qualquer posição:

```text
corrigir cálculo de juros #api !alta @hoje +sprint ^DEV-142 ~backend
```

| Marcador | Vira |
| --- | --- |
| `#repo` | repositório (por prefixo: `#api` → `api-pagamentos`) |
| `@hoje` `@sexta` `@12/10` | prazo |
| `!alta` `!!` `!!!` `!baixa` | prioridade |
| `+workspace` | workspace |
| `^DEV-142` | card do Notion |
| `~tag` | etiqueta livre |

Dentro de uma sessão aberta pelo AIC, `/aic:tarefa texto` cria a tarefa já vinculada à sessão.

### Abrir card

`Ctrl+Shift+J`, cole o link do card. O AIC cria um worktree por repositório (`<repo>.worktrees/<branch>`, branch com o nome do card), abre um Claude em cada, um shell e as notas. O plugin `aic` dá ao Claude as skills para puxar o contexto do card e da thread do Slack, quebrar o trabalho entre repositórios e fechar a entrega.

### Layne (assistente)

O Layne é um Claude sempre aberto num painel à direita (`Ctrl+B L` ou `Ctrl+Alt+L`). Você pede em português, digitando ou falando, e ele mexe no AIC por você: cria workspaces, anota tarefas, abre sessões do Claude num repo ou worktree com a primeira mensagem já escrita, abre um card do Notion montando o bloco.

> "cria um workspace pessoal e anota pra eu lembrar: renovar a CNH sexta, comprar ração"
> "abre uma sessão com o card DEV-142 no projeto api"

- **Voz**: `Ctrl+B m` (ou `Ctrl+Alt+V`, ou o microfone no painel) começa a gravar; de novo para, transcreve e manda. `Esc` no painel descarta. Fora do app, `aic voz` faz o mesmo (bom pra um atalho do i3: `bindsym $mod+v exec aic voz`).
- **Microfone e transcrição**: em Config → Layne. Escolha o mic da lista (ou o padrão do sistema) e quem transcreve: **whisper.cpp** local (nada sai da máquina; precisa do `whisper-cli` e de um modelo `ggml-*.bin`), uma **API** no formato da OpenAI (OpenAI ou Groq; chave no `~/.config/aic/.env`) ou um **comando seu** (`{file}` e `{lang}`).
- **A conversa continua**: a sessão tem id fixo e volta com `--resume`. "Conversa nova" (Config → Layne) começa outra.
- **Como ele opera**: pelo `aic` de linha de comando (agente `aic:layne` + skill `aic:layne` do plugin), que pede ao app aberto por uma caixa de entrada em `~/.local/state/aic/inbox/`. Os mesmos comandos servem pra você ou pra scripts:

```bash
aic ws new 'Pessoal' --go              # cria workspace (e vai pra ele)
aic ws go pess                          # troca (nome ou começo)
aic add --ws Pessoal 'renovar CNH @sexta'
aic session --repo api --branch fix/juros -- 'corrija o cálculo de juros'
aic card DEV-142 --auto --repos api     # monta o bloco sem perguntar
aic sessions | aic ws ls | aic repos    # o que está aberto
aic layne --send 'o que tenho pra hoje?'
```

### tmux por baixo

O AIC sobe um servidor tmux só dele, com config própria em `~/.local/share/aic/tmux.conf` (sem barra nem atalhos, invisível). O seu tmux e o seu `~/.tmux.conf` não são tocados. Dá para desligar no módulo **Persistência (tmux)**.

### Sons

Ao terminar uma sessão o AIC toca um som (e, se quiser, outro quando ela pede permissão). Há sons **sintetizados** pelo próprio app (acorde grunge, microfonia, sino, pop) e qualquer `.mp3/.ogg/.wav` que você puser em `~/.local/share/aic/sounds/`.

### Módulos

Em Config → Módulos: tmux, workspaces, Layne, abrir card, board, notas, worktrees, tokens, Notion, Slack, barra de status, dicas de atalho, mascote, fundo animado, som e notificações.

O manual completo, com todos os atalhos, fica dentro do app (`F1`) e em [`docs/MANUAL.md`](docs/MANUAL.md).

## Configuração

| Arquivo | O quê |
| --- | --- |
| `~/.config/aic/config.toml` | preferências (visualização, prefixo, fonte, módulos, sons…). Tudo tem padrão; o app grava nele pela tela de Config. |
| `~/.config/aic/keybinds.toml` | seus atalhos (`acao = "Ctrl+Shift+N"`, vazio desliga) e as teclas do prefixo (tabela `[prefix]`) |
| `~/.config/aic/.env` | tokens e ids das integrações (veja abaixo) |
| `~/.local/share/aic/` | banco de dados, sons, plugin, tmux.conf |
| `~/.local/state/aic/` | eventos dos hooks e snapshots da statusline |

### `.env`

Opcional. Copie o modelo e preencha só o que usar:

```bash
mkdir -p ~/.config/aic
cp .env.example ~/.config/aic/.env
chmod 600 ~/.config/aic/.env
```

| Variável | Para quê |
| --- | --- |
| `NOTION_TOKEN` | token de integração do Notion (só para o conector "Token") |
| `AIC_NOTION_TASKS_DB` | id da database de cards |
| `AIC_NOTION_SPRINTS_DB` | id da database de sprints |
| `AIC_NOTION_ME` | você no Notion (e-mail ou id), para "só meus cards" |
| `AIC_NOTION_KEY_PREFIX` | prefixo da chave dos cards (ex: `DEV`) |
| `AIC_REPOS_DIRS` | pastas com repositórios, separadas por `:` (aceita `~`) |
| `SLACK_WORKSPACE_URL` | URL do workspace do Slack |

O arquivo é lido ao abrir o app; variáveis que já estão no ambiente têm prioridade sobre ele, e ele tem prioridade sobre o `config.toml`.

Notion e Slack funcionam de dois jeitos: pelo **conector Claude** (padrão: usa os conectores já ligados na sua conta do Claude, sem token) ou pelo **conector Token** (API direta do Notion, mais rápido).

## Desenvolvimento

```bash
cargo run -p aic-app                         # roda do código
cargo test --workspace                       # testes
cargo test -p aic-app --test snapshots       # renderiza as telas sem janela em target/snapshots/
```

Estrutura: `crates/aic-core` (modelo, store SQLite, captura rápida, git, Notion, uso), `crates/aic-term` (terminal embutido), `crates/aic-app` (UI, visual em `brand.rs`, CLI), `claude/` (plugin do Claude Code), `docs/MANUAL.md` (o manual que aparece no app).

## Créditos

- O terminal embutido (`crates/aic-term`) é derivado do [egui_term](https://github.com/Harzu/egui_term) (MIT, © 2024 Ilia Shvyrialkin; licença em [`crates/aic-term/LICENSE-egui_term`](crates/aic-term/LICENSE-egui_term)), sobre o [alacritty_terminal](https://github.com/alacritty/alacritty).
- O visual do AIC é só **inspirado** no grunge de Seattle e em Alice in Chains: desenho, mascote e sons sintetizados são originais, sem logos, capas ou gravações da banda. Não há relação com a banda.
- Fonte recomendada: [JetBrainsMono Nerd Font](https://www.nerdfonts.com/) (OFL).

## Licença

[MIT](LICENSE) © Vinicius Gonçalves
