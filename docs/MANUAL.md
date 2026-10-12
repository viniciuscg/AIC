# AIC — o manual

> down in a hole, up in the stack

O AIC é uma central para trabalhar com vários Claude Code ao mesmo tempo. Cada contexto de trabalho (um card, um bug, uma ideia) vira um **bloco** com seus terminais, notas, tarefas e o card do Notion. Por baixo, os terminais rodam num **tmux** próprio: fechar a janela não mata nada.

O visual é inspirado no grunge de Seattle e em Alice in Chains. É só inspiração: o desenho é original, sem logos nem capas da banda.

## Primeiros passos

1. Na primeira vez, o assistente pergunta a visualização, os módulos, as pastas dos seus repositórios, o prefixo e a tecla do catálogo. Tudo muda depois em **Config** (**Ctrl+B ,**).
2. **Ctrl+B c** abre um Claude (escolha a pasta ou um worktree). **Ctrl+B %** abre um shell ao lado, na mesma pasta.
3. Trabalhe no terminal. O prefixo funciona com o terminal focado, então você não precisa soltar o teclado.
4. Esqueceu um atalho? **Ctrl+Space** (ou **Ctrl+B ?**) abre o catálogo com todos, agrupados, e mostra ao lado de cada um o atalho direto e o acorde do prefixo.

## O prefixo (o jeito tmux)

Aperte **Ctrl+B**, solte e aperte a tecla da ação. Nada de menu: a barra de cima acende **Ctrl+B** em amarelo enquanto espera, e a barra de dicas mostra o que dá pra apertar.

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
| `i` / `e` | painel do bloco / escrever nas notas |
| `C` | abrir card |
| `t` / `T` | nova tarefa / vincular tarefa ao terminal |
| `n` / `p` | próxima / anterior aba da barra |
| `s` `b` `g` `u` `,` | abas Sessões, Board, Worktrees, Tokens, Config |
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

**Ctrl+B x** fecha a coisa mais específica que estiver em foco: no board, o cartão aberto; numa aba que não é Sessões, a própria aba (volta pras sessões); nas sessões, o terminal em foco (pede confirmação) ou, se o bloco estiver vazio, o bloco.

### Mudar o prefixo e as teclas

O prefixo muda em **Config › Atalhos** (Ctrl+A, por exemplo). Cada ação tem duas colunas lá: o atalho direto e a tecla depois do prefixo, as duas editáveis. No arquivo `~/.config/aic/keybinds.toml` as teclas de prefixo ficam numa tabela própria:

```toml
[prefix]
close_focused = "q"
focus_left = ["h", "←"]
```

## A barra de cima

Uma linha, no espírito do i3bar:

- **workspaces** à esquerda, numerados: o atual fica aceso; os que têm alguém esperando você ficam roxos e mostram quantos. Clique pra trocar, clique direito pra renomear;
- **Ctrl+B** em amarelo quando o prefixo está armado;
- **abas** com a tecla do prefixo de cada uma (`s Sessões`, `b Board`…);
- à direita, em blocos: quem está aguardando, quem está trabalhando, custo de hoje, limites de 5h e da semana, e o relógio.

## Workspaces

Um workspace é um contexto inteiro: blocos, terminais, layout e tarefas. O que aparece na tela é sempre só o do workspace aceso na barra, inclusive o board e a paleta. Para ver outra coisa, troque de workspace: **Ctrl+B 1..9**, **Ctrl+B ( )**, **Ctrl+B Tab** pra voltar pro anterior, ou clique na barra.

Tarefas sem workspace (como os cards que vêm do Notion) aparecem em todos.

## Blocos

Um bloco é um contexto: vários terminais (Claude ou shell), notas em Markdown, tarefas e, se houver, o card do Notion e a thread do Slack.

- **Ctrl+B B** cria, **Ctrl+B R** renomeia, **Ctrl+B &** fecha (com todos os terminais dele).
- **Ctrl+B Space** troca o arranjo dos terminais: lado a lado → principal → empilhado → grade.
- **Ctrl+B i** mostra o painel do bloco (card, notas, tarefas); **Ctrl+B e** vai direto para as notas.
- **Alt+1** a **Alt+9** pulam para o bloco N.

### Abrir card

**Ctrl+B C**: cole o link de um card (Notion) e o AIC monta o bloco sozinho: um worktree por repositório envolvido, um Claude em cada, um shell e as notas. O Claude recebe o contexto do card pela skill `aic:contexto`.

## Visualizações

Troque com **Ctrl+B v** (ou em Config):

| Visualização | Como fica |
| --- | --- |
| **Blocos** | cada bloco com seus terminais, um embaixo do outro (padrão) |
| **Abas** | um bloco por aba, terminais divididos lado a lado; o jeito tmux |
| **Foco** | um terminal grande e a lista de sessões ao lado, com prévia |
| **Grade** | todos os terminais do workspace numa grade |

## Estados das sessões

O AIC acompanha cada Claude pelos hooks do Claude Code. Os estados têm nomes de música e o mascote é um pote de vidro com moscas:

| Estado | Nome | O que acontece no pote |
| --- | --- | --- |
| BOOT | ROOSTER | o pote surge do chão |
| WORKING | THEM BONES | moscas zunindo rápido, deixando rastro |
| IDLE | NUTSHELL | poucas moscas, devagar; uma pousada no vidro |
| WAITING | WOULD? | elas batem no vidro e o pote treme |
| DONE | NO EXCUSES | a tampa salta e as moscas escapam |
| GAME OVER | DOWN IN A HOLE | moscas caídas no fundo, vidro rachado |

**Ctrl+B a** pula para a próxima sessão que precisa de você (WAITING ou DONE), mesmo que esteja em outro workspace. A barra de baixo mostra a statusline da sessão em foco: modelo, contexto, branch, limites da conta e custo.

Os hooks, a statusline e o plugin `aic` só valem nas sessões abertas pelo AIC: o seu `~/.claude` não é alterado.

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
| `^DEV-142` | card do Notion |
| `~tag` | etiqueta livre |

Pelo terminal: `aic add "texto #repo @hoje"`, `aic tasks`, `aic done <id>`. Dentro de uma sessão do AIC, o comando `/aic:tarefa` cria a tarefa já vinculada. **Ctrl+B T** vincula uma tarefa à sessão em foco.

## Board

Kanban **A fazer → Fazendo → Revisão → Feito** (**Ctrl+B b**), com as tarefas do workspace atual e os cards do Notion. Tudo no teclado: `j/k` e `h/l` navegam, `Shift+H/L` movem o cartão, `Enter` abre a sessão da tarefa no worktree certo, `e` edita, `x` conclui, `Shift+P` promove para o Notion, `/` busca. **Ctrl+B x** fecha o cartão aberto.

## Worktrees

Aba **Ctrl+B g**. Os worktrees ficam em `<repo>.worktrees/<branch>`, ao lado do repo. `n` cria, `Enter` abre uma sessão nele, `Delete` remove (a branch fica). Tarefas com card geram branch com o nome do card (`DEV-142-corrigir-juros`).

## Tokens

Aba **Ctrl+B u**: limites da conta (janela de 5h e semana), uso por sessão e histórico de custo lido dos transcripts do Claude Code.

## Config

**Ctrl+B ,** abre; **Ctrl+B /** (ou `/` dentro da Config) vai direto pra busca. Digite parte do nome (`fonte`, `som`, `notion`, `workspace 3`) e só aparece o que bate, inclusive os atalhos. Acento não importa; `Esc` limpa.

## Sons e notificações

Quando uma sessão termina, o AIC toca um som e manda uma notificação do sistema; quando pede permissão, avisa também (o som de atenção é opcional). Por padrão só avisa se a janela estiver sem foco ou a sessão não estiver visível.

Sons disponíveis: os **sintetizados** (Acorde grunge, o padrão; Microfonia; Sino; Pop) e qualquer `.mp3/.ogg/.wav` que você colocar em `~/.local/share/aic/sounds/`. Escolha em Config → Som.

## Módulos

Tudo além dos terminais é um módulo que liga e desliga em Config → Módulos: tmux, workspaces, abrir card, board, notas, worktrees, tokens, Notion, Slack, barra de status, dicas de atalho, mascote, fundo animado, som e notificações. Desligar um módulo some com a aba, os atalhos e as ações dele.

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
| Ctrl+Shift+I | Painel do bloco (card, notas, tarefas) |
| Ctrl+Shift+E | Escrever nas notas do bloco |
| Ctrl+Shift+J | Abrir card |
| Ctrl+Shift+Space | Nova tarefa (captura rápida) |
| Ctrl+Shift+K | Vincular tarefa à sessão |
| Ctrl+Shift+O | Lista de workspaces |
| Ctrl+Shift+PageDown / PageUp | Próximo / anterior workspace |
| Ctrl+Shift+S / B / G / U | Abas Sessões / Board / Worktrees / Tokens |
| Ctrl+, | Configurações e atalhos |
| F1 | Este manual |
| Ctrl+Shift+P | Paleta de comandos |
| Ctrl+Shift+= / - / 0 | Aumentar / diminuir / restaurar a fonte |
| Ctrl+Shift+D | Mostrar/esconder a lista lateral |
| Ctrl+Shift+H | Mostrar/esconder a barra de dicas |
| Ctrl+Shift+Q | Sair |

No terminal: **Ctrl+Shift+C / Ctrl+Shift+V** copiam e colam, **Shift+Enter** quebra linha no prompt do Claude, **Ctrl+clique** abre link.

Todos os atalhos são editáveis em **Config › Atalhos** (ou em `~/.config/aic/keybinds.toml`).

## Créditos

Visual e mascote: **Vinicius**. A trilha sonora fica por sua conta.
