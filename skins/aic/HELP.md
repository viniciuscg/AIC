# AIC — o manual

> down in a hole, up in the stack

O AIC é uma central para trabalhar com vários Claude Code ao mesmo tempo. Cada contexto de trabalho (um card, um bug, uma ideia) vira um **bloco** com seus terminais, notas, tarefas e o card do Notion. Por baixo, os terminais rodam num **tmux** próprio: fechar a janela não mata nada.

Este é o tema padrão, inspirado no grunge de Seattle e em Alice in Chains. É só inspiração: o desenho é original, sem logos nem capas da banda. O outro tema, **Plankton**, muda só a cara (veja a aba dele).

## Primeiros passos

1. Na primeira vez, o assistente pergunta a visualização, o tema, as pastas dos seus repositórios, a tecla líder e os módulos. Tudo muda depois em **Config** (**Ctrl+,**).
2. Crie um bloco com **Ctrl+Shift+Enter** e abra um Claude nele com **Ctrl+Shift+N** (escolha a pasta ou um worktree). **Ctrl+Shift+T** abre um shell comum na mesma pasta, ao lado.
3. Trabalhe no terminal. Para usar os atalhos do app, solte o teclado com **Ctrl+Shift+Esc**; clique no terminal para devolver.
4. Esqueceu um atalho? Aperte a **tecla líder** (padrão **Ctrl+Space**) e aparece o menu com todos, agrupados. A paleta de comandos (**Ctrl+Shift+P**) também faz tudo.

## Blocos

Um bloco é um contexto: vários terminais (Claude ou shell), notas em Markdown, tarefas e, se houver, o card do Notion e a thread do Slack.

- **Ctrl+Shift+Enter** cria, **Ctrl+Shift+F2** renomeia, **Ctrl+Shift+X** fecha (com todos os terminais dele).
- **Ctrl+Shift+\\** troca o arranjo dos terminais: lado a lado → principal → empilhado → grade.
- **Ctrl+Shift+I** mostra o painel do bloco (card, notas, tarefas); **Ctrl+Shift+E** vai direto para as notas.
- **Alt+1** a **Alt+9** pulam para o bloco N.

### Abrir card

**Ctrl+Shift+J**: cole o link de um card (Notion) e o AIC monta o bloco sozinho: um worktree por repositório envolvido, um Claude em cada, um shell e as notas. O Claude recebe o contexto do card pela skill `aic:contexto`.

## Visualizações

Troque com **Ctrl+Shift+V** (ou em Config):

| Visualização | Como fica |
| --- | --- |
| **Blocos** | cada bloco com seus terminais, um embaixo do outro (padrão) |
| **Abas** | um bloco por aba, terminais divididos lado a lado; o jeito tmux |
| **Foco** | um terminal grande e a lista de sessões ao lado, com prévia |
| **Grade** | todos os terminais do workspace numa grade |

**Ctrl+Shift+M** maximiza o terminal em foco; **Ctrl+Shift+D** esconde a lista lateral; **Ctrl+Shift+← → ↑ ↓** move o foco.

## Estados das sessões

O AIC acompanha cada Claude pelos hooks do Claude Code. Neste tema os estados têm nomes de música e o mascote é um pote de vidro com moscas:

| Estado | Nome | O que acontece no pote |
| --- | --- | --- |
| BOOT | ROOSTER | o pote surge do chão |
| WORKING | THEM BONES | moscas zunindo rápido, deixando rastro |
| IDLE | NUTSHELL | poucas moscas, devagar; uma pousada no vidro |
| WAITING | WOULD? | elas batem no vidro e o pote treme |
| DONE | NO EXCUSES | a tampa salta e as moscas escapam |
| GAME OVER | DOWN IN A HOLE | moscas caídas no fundo, vidro rachado |

**Ctrl+Shift+A** pula para a próxima sessão que precisa de você (WAITING ou DONE). A barra de baixo mostra a statusline da sessão em foco: modelo, contexto, branch, limites da conta e custo.

Os hooks, a statusline e o plugin `aic` só valem nas sessões abertas pelo AIC: o seu `~/.claude` não é alterado.

## tmux por baixo

Os terminais rodam num servidor tmux só do AIC (config em `~/.local/share/aic/tmux.conf`, sem barra nem atalhos próprios). Fechar a janela ou o app cair não mata os Claudes: ao abrir de novo, o AIC reconecta. Dá para desligar no módulo **Persistência (tmux)**.

## Captura rápida

**Ctrl+Shift+Space** (ou `t` com o terminal sem foco) abre a captura rápida. Uma linha vira tarefa; os marcadores podem estar em qualquer posição:

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

Pelo terminal: `aic add "texto #repo @hoje"`, `aic tasks`, `aic done <id>`. Dentro de uma sessão do AIC, o comando `/aic:tarefa` cria a tarefa já vinculada. **Ctrl+Shift+K** vincula uma tarefa à sessão em foco.

## Board

Kanban **A fazer → Fazendo → Revisão → Feito** (**Ctrl+Shift+B**), com tarefas locais e cards do Notion. Tudo no teclado: `j/k` e `h/l` navegam, `Shift+H/L` movem o cartão, `Enter` abre a sessão da tarefa no worktree certo, `e` edita, `x` conclui, `Shift+P` promove para o Notion, `/` busca.

## Worktrees

Aba **Ctrl+Shift+G**. Os worktrees ficam em `<repo>.worktrees/<branch>`, ao lado do repo. `n` cria, `Enter` abre uma sessão nele, `Delete` remove (a branch fica). Tarefas com card geram branch com o nome do card (`DEV-142-corrigir-juros`).

## Tokens

Aba **Ctrl+Shift+U**: limites da conta (janela de 5h e semana), uso por sessão e histórico de custo lido dos transcripts do Claude Code.

## Sons e notificações

Quando uma sessão termina, o AIC toca um som e manda uma notificação do sistema; quando pede permissão, avisa também (o som de atenção é opcional). Por padrão só avisa se a janela estiver sem foco ou a sessão não estiver visível.

Sons disponíveis: os **sintetizados** (Acorde grunge, o padrão deste tema; Microfonia; Sino; Pop), os **baixados** pelo `install.sh` e qualquer `.mp3/.ogg/.wav` que você colocar em `~/.local/share/aic/sounds/`. Escolha em Config → Som.

## Módulos

Tudo além dos terminais é um módulo que liga e desliga em Config → Módulos (globalmente ou só neste tema): tmux, workspaces, abrir card, board, notas, worktrees, tokens, Notion, Slack, barra de status, dicas de atalho, mascote, fundo animado, som e notificações. Desligar um módulo some com a aba, os atalhos e as ações dele.

## Fechar e reabrir

Workspaces, blocos e layout são salvos o tempo todo. Ao sair (**Ctrl+Shift+Q**) você escolhe salvar o scrollback ou não; no próximo início o AIC oferece reabrir (`claude --resume`). Para começar limpo uma vez: `aic --fresh`.

## Visual deste tema

- **Mascote**: um pote de vidro com moscas e tampa de metal enferrujada.
- **Fundo**: granulação de filme, poeira, riscos de película e raios de sol empoeirados.
- **Letreiro**: `AIC` com letras tortas, impressão desalinhada e riscos, como um cartaz xerocado.
- **Cantos quase retos** e sombra dura de 2px.
- **Variantes** (troque com **Ctrl+Shift+Y**): **dirt**, terra seca, ferrugem e mostarda queimada (padrão); **jar**, sépia de papel velho e tinta marrom.

## Atalhos

| Atalho | Ação |
| --- | --- |
| Ctrl+Shift+N | Novo Claude no bloco (escolhe pasta/worktree) |
| Ctrl+Shift+T | Novo terminal na mesma pasta (split) |
| Ctrl+Shift+W | Fechar sessão |
| Ctrl+Shift+R | Renomear sessão |
| Ctrl+PageDown / Ctrl+PageUp | Próxima / anterior sessão |
| Ctrl+Shift+A | Ir pra próxima que precisa de você |
| Ctrl+Shift+← → ↑ ↓ | Mover o foco |
| Alt+1 … Alt+9 | Ir pro bloco/sessão N |
| Ctrl+Shift+L | Alternar foco / grade |
| Ctrl+Shift+V | Próxima visualização |
| Ctrl+Shift+M | Maximizar sessão (zoom) |
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
| Ctrl+Shift+O | Trocar de workspace |
| Ctrl+Shift+PageDown / PageUp | Próximo / anterior workspace |
| Ctrl+Shift+S / B / G / U | Abas Sessões / Board / Worktrees / Tokens |
| Ctrl+, | Configurações e atalhos |
| F1 | Ajuda do tema |
| Ctrl+Shift+P | Paleta de comandos |
| Ctrl+Shift+= / - / 0 | Aumentar / diminuir / restaurar a fonte |
| Ctrl+Shift+Y | Próxima variante de tema |
| Ctrl+Shift+D | Mostrar/esconder a lista lateral |
| Ctrl+Shift+Q | Sair |

No terminal: **Ctrl+Shift+C / Ctrl+Shift+V** copiam e colam, **Shift+Enter** quebra linha no prompt do Claude, **Ctrl+clique** abre link.

Todos os atalhos são editáveis em **Config › Atalhos** (ou em `~/.config/aic/keybinds.toml`); a lista com os seus atalhos atuais está logo abaixo.

## Créditos

Tema e mascote: **Vinicius**. A trilha sonora fica por sua conta.
