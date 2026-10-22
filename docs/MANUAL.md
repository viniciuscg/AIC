# AIC — o manual

> down in a hole, up in the stack

O AIC é uma central para trabalhar com vários Claude Code ao mesmo tempo. Cada contexto de trabalho (um card, um bug, uma ideia) vira um **bloco** com seus terminais e tarefas, e cada sessão tem a sua nota. Por baixo, os terminais rodam num **tmux** próprio: fechar a janela não mata nada.

O visual é sóbrio e inspirado no grunge de Seattle e em Alice in Chains (a paleta, os nomes de música nos estados). É só inspiração: o desenho é original, sem logos nem capas da banda.

## Primeiros passos

1. Na primeira vez, o assistente pergunta a visualização, os módulos, as pastas dos seus repositórios, o prefixo e a tecla do catálogo. Tudo muda depois em **Config** (**Ctrl+B ,**).
2. **Ctrl+B c** abre um Claude (escolha a pasta ou um worktree). **Ctrl+B %** abre um shell ao lado, na mesma pasta.
3. Trabalhe no terminal. O prefixo funciona com o terminal focado, então você não precisa soltar o teclado.
4. Esqueceu um atalho? **Ctrl+Space** (ou **Ctrl+B ?**) abre o catálogo com todos, agrupados, e mostra ao lado de cada um o atalho direto e o acorde do prefixo.

## O prefixo (o jeito tmux)

Aperte **Ctrl+B**, solte e aperte a tecla da ação. Nada de menu: a barra de cima acende **Ctrl+B** em amarelo enquanto espera. Esqueceu a tecla? **Ctrl+B ?** mostra todas.

| Tecla | Ação |
| --- | --- |
| `c` | novo Claude |
| `%` ou `"` | novo terminal ao lado (split) |
| `x` | fecha o que está em foco (veja abaixo) |
| `&` | fecha o bloco inteiro |
| `h j k l` ou setas | move o foco entre terminais |
| `o` / `O` | próximo / anterior terminal |
| `z` | maximiza / restaura o terminal |
| `a` | vai pra próxima sessão que precisa de você |
| `Space` | muda o arranjo dos terminais do bloco |
| `v` | troca a visualização (blocos → abas → foco → grade) |
| `r` / `R` | renomeia o terminal / o bloco |
| `B` | novo bloco |
| `{` / `}` | sobe / desce o bloco |
| `e` | notas da sessão em foco (abre já escrevendo; de novo fecha) |
| `i` | painel do bloco (tarefas) |
| `t` | nova tarefa |
| `n` / `p` | próxima / anterior aba da barra |
| `s` `g` `G` `u` `,` | abas Sessões, Worktrees, Changes, Tokens, Config |
| `/` | pesquisar nas configurações |
| `1` a `9` | workspace N |
| `(` / `)` | workspace anterior / próximo |
| `Tab` | volta pro workspace de antes (vai e volta) |
| `w` / `W` | lista de workspaces / novo workspace |
| `$` | renomeia o workspace |
| `:` | paleta de comandos |
| `?` | catálogo de atalhos |
| `[` | solta o teclado do terminal |
| `Ctrl+B` | manda um Ctrl+B de verdade pro terminal |
| `Esc` | cancela |

### Fechar o que está em foco

**Ctrl+B x** fecha a coisa mais específica que estiver em foco: numa aba que não é Sessões, a própria aba (volta pras sessões); nas sessões, o terminal em foco (pede confirmação) ou, se o bloco estiver vazio, o bloco.

### Mudar o prefixo e as teclas

O prefixo muda em **Config › Atalhos** (Ctrl+A, por exemplo). Cada ação tem duas colunas lá: o atalho direto e a tecla depois do prefixo, as duas editáveis. No arquivo `~/.config/aic/keybinds.toml` as teclas de prefixo ficam numa tabela própria:

```toml
[prefix]
close_focused = "q"
focus_left = ["h", "←"]
```

## A barra de cima

Uma linha só, no espírito do i3bar (não há barra embaixo):

- **workspaces** à esquerda, numerados: o atual fica aceso; os que têm alguém esperando você ficam roxos e mostram quantos. Clique pra trocar, clique direito pra renomear;
- **Ctrl+B** em amarelo quando o prefixo está armado;
- **abas** com a tecla do prefixo de cada uma (`s Sessões`, `g Worktrees`…); a Ajuda abre no **F1**;
- à direita, **a janela em foco**: o estado (com nome de música), o nome, o modelo, quanto do contexto já foi usado, a branch (e `±N` arquivos mudados) e o custo da sessão; e o ícone do Layne.

Esses dados vêm da statusline do Claude Code: o AIC injeta nas sessões dele uma statusline que só grava os números e não imprime nada, então o terminal do Claude fica limpo. Fora do AIC, a sua statusline continua igual. Os limites da conta (5h e semana) ficam na aba **Tokens**.

## Layne (assistente)

O Layne é um Claude sempre aberto num painel à direita (`Ctrl+B L` ou `Ctrl+Alt+L`). Você pede em português, digitando ou falando, e ele mexe no AIC por você: cria workspaces, anota tarefas, abre sessões do Claude num repo ou worktree com a primeira mensagem já escrita.

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
aic sessions | aic ws ls | aic repos    # o que está aberto
aic layne --send 'o que tenho pra hoje?'
```

## Workspaces

Um workspace é um contexto inteiro: blocos, terminais, layout e tarefas. O que aparece na tela é sempre só o do workspace aceso na barra, inclusive a paleta. Para ver outra coisa, troque de workspace: **Ctrl+B 1..9**, **Ctrl+B ( )**, **Ctrl+B Tab** pra voltar pro anterior, ou clique na barra. **Ctrl+B W** cria um.

## Blocos

Um bloco é um contexto: vários terminais (Claude ou shell) e as tarefas ligadas a eles.

- **Ctrl+B B** cria, **Ctrl+B R** renomeia, **Ctrl+B &** fecha (com todos os terminais dele).
- **Ctrl+B Space** troca o arranjo dos terminais: lado a lado → principal → empilhado → grade.
- **Ctrl+B i** mostra o painel do bloco, com as tarefas dele (e um campo pra criar mais).
- **Alt+1** a **Alt+9** pulam para o bloco N.

## Notas da sessão

Cada sessão (Claude ou terminal) tem a sua nota em Markdown. **Ctrl+B e** (ou **Ctrl+Shift+E**) abre o painel à direita já com o cursor na nota da sessão em foco; **Ctrl+B e** de novo fecha.

- Salva sozinha, a cada tecla, junto da sessão. Reabrindo as sessões salvas, as notas voltam com elas; fechar uma sessão, ou sair sem salvar o cache, apaga a nota junto.
- Trocar de sessão troca a nota.
- **Esc** devolve o teclado pro terminal (o painel continua aberto).
- Notas de bloco de versões antigas passam pra sessão em foco de cada bloco ao reabrir.

## Visualizações

Troque com **Ctrl+B v** (ou em Config):

| Visualização | Como fica |
| --- | --- |
| **Blocos** | cada bloco com seus terminais, um embaixo do outro (padrão) |
| **Abas** | um bloco por aba, terminais divididos lado a lado; o jeito tmux |
| **Foco** | um terminal grande e a lista de sessões ao lado, com prévia |
| **Grade** | todos os terminais do workspace numa grade |

## Estados das sessões

O AIC acompanha cada Claude pelos hooks do Claude Code. Os estados têm nomes de música:

| Estado | Nome | Quando |
| --- | --- | --- |
| BOOT | ROOSTER | abrindo |
| WORKING | THEM BONES | o Claude está trabalhando |
| IDLE | NUTSHELL | esperando você digitar |
| WAITING | WOULD? | pediu permissão ou fez uma pergunta |
| DONE | NO EXCUSES | terminou e você ainda não olhou |
| GAME OVER | DOWN IN A HOLE | o processo morreu |

Quem precisa de você (WAITING ou DONE) ganha um anel em volta da bolinha e a moldura do terminal na cor do estado. **Ctrl+B a** pula para a próxima, mesmo que esteja em outro workspace. A barra de cima mostra o estado e os números da sessão em foco.

Os hooks, a statusline (que só grava os números, sem imprimir no terminal) e o plugin `aic` só valem nas sessões abertas pelo AIC: o seu `~/.claude` não é alterado.

## tmux por baixo

Os terminais rodam num servidor tmux só do AIC (config em `~/.local/share/aic/tmux.conf`, sem barra nem atalhos próprios). Fechar a janela ou o app cair não mata os Claudes: ao abrir de novo, o AIC reconecta. Dá para desligar no módulo **Persistência (tmux)**.

## Captura rápida

**Ctrl+B t** (ou `t` com o terminal sem foco) abre a captura rápida. Uma linha vira tarefa; os marcadores podem estar em qualquer posição:

```text
corrigir cálculo de juros #api !alta @hoje +sprint ^DEV-142 ~backend
```

| Marcador | Vira |
| --- | --- |
| `#repo` | repositório (por prefixo: `#api` → `api-pagamentos`) |
| `@hoje` `@sexta` `@12/10` | prazo |
| `!alta` `!!` `!!!` `!baixa` | prioridade |
| `+workspace` | workspace (por prefixo do nome) |
| `^DEV-142` | chave do card (vira o começo do nome da branch) |
| `~tag` | etiqueta livre |

Na captura, `Alt+L` vincula (ou não) a tarefa ao terminal em foco. As tarefas aparecem no painel do bloco (**Ctrl+B i**), onde um clique marca como feita.

Pelo terminal: `aic add "texto #repo @hoje"`, `aic tasks`, `aic done <id>`. Dentro de uma sessão do AIC, o comando `/aic:tarefa` cria a tarefa já vinculada.

## Worktrees

Aba **Ctrl+B g**. Os worktrees ficam em `<repo>.worktrees/<branch>`, ao lado do repo. `n` cria, `Enter` abre uma sessão nele, `Delete` remove (a branch fica). Tarefas com card geram branch com o nome do card (`DEV-142-corrigir-juros`).

## Changes

Aba **Ctrl+B G**: os arquivos mudados do repo do terminal em foco e o diff de cada um, como o Source Control do VS Code. `j/k` escolhe o arquivo, `s`/`u` faz stage/unstage, `r` atualiza; o commit sai da caixa de mensagem no topo.

## Tokens

Aba **Ctrl+B u**: limites da conta (janela de 5h e semana), uso por sessão e histórico de custo lido dos transcripts do Claude Code.

## Config

**Ctrl+B ,** abre; **Ctrl+B /** (ou `/` dentro da Config) vai direto pra busca. Digite parte do nome (`fonte`, `som`, `prefixo`, `workspace 3`) e só aparece o que bate, inclusive os atalhos. Acento não importa; `Esc` limpa.

## Sons e notificações

Quando uma sessão termina, o AIC toca um som e manda uma notificação do sistema; quando pede permissão, avisa também (o som de atenção é opcional). Por padrão só avisa se a janela estiver sem foco ou a sessão não estiver visível.

Sons disponíveis: os **sintetizados** (Acorde grunge, o padrão; Microfonia; Sino; Pop) e qualquer `.mp3/.ogg/.wav` que você colocar em `~/.local/share/aic/sounds/`. Escolha em Config → Som.

## Módulos

Tudo além dos terminais é um módulo que liga e desliga em Config → Módulos: tmux, workspaces, Layne, tarefas, notas, worktrees, Changes, tokens, catálogo de atalhos, som e notificações. Desligar um módulo some com a aba, os atalhos e as ações dele.

## Fechar e reabrir

Workspaces, blocos e layout são salvos o tempo todo. Ao sair (**Ctrl+Shift+Q**) você escolhe salvar o scrollback ou não; no próximo início o AIC oferece reabrir (`claude --resume`). Para começar limpo uma vez: `aic --fresh`.

## Atalhos diretos

Quem prefere uma combinação só continua tendo os atalhos diretos:

| Atalho | Ação |
| --- | --- |
| Ctrl+Shift+N | Novo Claude no bloco (escolhe pasta/worktree) |
| Ctrl+Shift+T | Novo terminal na mesma pasta (split) |
| Ctrl+Shift+W | Fechar terminal |
| Ctrl+Shift+R | Renomear terminal |
| Ctrl+PageDown / Ctrl+PageUp | Próximo / anterior terminal |
| Ctrl+Shift+A | Ir pra próxima que precisa de você |
| Ctrl+Shift+← → ↑ ↓ | Mover o foco |
| Alt+1 … Alt+9 | Ir pro bloco N |
| Ctrl+Alt+1 … Ctrl+Alt+9 | Ir pro workspace N |
| Ctrl+Tab / Ctrl+Shift+Tab | Próxima / anterior aba da barra |
| Ctrl+Shift+F | Próxima visualização |
| Ctrl+Shift+L | Alternar foco / grade |
| Ctrl+Shift+M | Maximizar terminal |
| Ctrl+Shift+Esc | Soltar o teclado do terminal |
| Ctrl+Shift+Enter | Novo bloco |
| Ctrl+Shift+X | Fechar bloco |
| Ctrl+Shift+F2 | Renomear bloco |
| Ctrl+Shift+\\ | Arranjo dos terminais do bloco |
| Ctrl+Shift+I | Painel do bloco (tarefas) |
| Ctrl+Shift+E | Notas da sessão em foco (abre/fecha) |
| Ctrl+Shift+Space | Nova tarefa (captura rápida) |
| Ctrl+Shift+O | Lista de workspaces |
| Ctrl+Shift+PageDown / PageUp | Próximo / anterior workspace |
| Ctrl+Shift+S / G / U | Abas Sessões / Worktrees / Tokens |
| Ctrl+Alt+G | Aba Changes |
| Ctrl+, | Configurações e atalhos |
| F1 | Este manual |
| Ctrl+Shift+P | Paleta de comandos |
| Ctrl+Shift+= / - / 0 | Aumentar / diminuir / restaurar a fonte |
| Ctrl+Shift+D | Mostrar/esconder a lista lateral |
| Ctrl+Shift+Q | Sair |

No terminal: **Ctrl+Shift+C / Ctrl+Shift+V** copiam e colam, **Shift+Enter** quebra linha no prompt do Claude, **Ctrl+clique** abre link.

Todos os atalhos são editáveis em **Config › Atalhos** (ou em `~/.config/aic/keybinds.toml`).

## Créditos

Visual: **Vinicius**. A trilha sonora fica por sua conta.
