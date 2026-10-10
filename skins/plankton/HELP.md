# Plankton — o manual

> Central de vibecode: vários Claude Code rodando lado a lado, com tarefas, worktrees e tokens no mesmo lugar. O plano infalível pra roubar a fórmula do código.

O Plankton é o **pai** de todos os forks. Tudo que está aqui vale para os forks também; cada fork só muda a cara (tema, mascote, nomes dos estados).

## Primeiros passos

1. Abra uma sessão do Claude com **Ctrl+Shift+N** (ou um terminal comum com **Ctrl+Shift+T**).
2. Escolha a pasta: os repositórios de `repos/`, a raiz da CLI ou a sua home.
3. Trabalhe normalmente no terminal. Quando quiser usar os atalhos do app, solte o teclado com **Ctrl+Shift+Esc**; clique no terminal para devolver.
4. Se esquecer um atalho, abra a paleta de comandos com **Ctrl+Shift+P**: dá pra fazer tudo por ela.

## Sessões

Cada sessão é um Claude (ou shell) num terminal embutido. O Plankton acompanha o estado de cada uma pelos hooks do Claude Code:

| Estado | Nome no Plankton | Quando |
| --- | --- | --- |
| BOOT | LIGANDO | a sessão está abrindo |
| WORKING | TRAMANDO | o Claude está trabalhando |
| IDLE | À ESPREITA | esperando você digitar |
| WAITING | KAREN?! | pediu permissão ou fez uma pergunta |
| DONE | MUAHAHA! | terminou e você ainda não viu |
| GAME OVER | DERROTADO | o processo morreu |

- **Modo foco**: lista lateral + uma sessão grande. **Modo grade**: todas lado a lado. Alterne com **Ctrl+Shift+L**.
- **Ctrl+Shift+A** pula direto pra próxima sessão que precisa de você (WAITING ou DONE).
- **Alt+1** a **Alt+9** vão direto para a sessão pelo número.
- **Ctrl+Shift+M** maximiza a sessão em foco; **Ctrl+Shift+D** esconde a lista lateral.
- A barra de baixo mostra a statusline do Claude da sessão em foco: modelo, contexto, limites da conta, custo e agentes.

Os hooks e a statusline só valem nas sessões abertas pelo Plankton: o seu `~/.claude` não é alterado.

## Workspaces

Workspaces agrupam sessões (ex: um por sprint ou por projeto). **Ctrl+Shift+O** troca de workspace; **Ctrl+Shift+PageDown/PageUp** vai pro próximo/anterior.

## Board e tarefas

O Board é um kanban com **A fazer → Fazendo → Revisão → Feito**, misturando tarefas locais e cards do Notion. Tudo no teclado: `j/k` navegam, `Shift+H/L` movem o cartão, `Enter` abre a sessão da tarefa no worktree certo.

### Captura rápida

**Ctrl+Shift+Space** abre a captura rápida (ou `t` na aba Sessões, com o terminal sem foco). Uma linha de texto vira tarefa; os marcadores podem estar em qualquer posição:

```text
corrigir cálculo de juros #api !alta @hoje +sprint ^DEV-1256 ~backend
```

| Marcador | Vira |
| --- | --- |
| `#repo` | repositório (por prefixo: `#api` → `api-pagamentos`) |
| `@hoje` `@sexta` `@12/10` | prazo |
| `!alta` `!!` `!!!` `!baixa` | prioridade |
| `+workspace` | workspace (por prefixo do nome) |
| `^DEV-1256` | card do Notion |
| `~tag` | etiqueta livre |

Também dá pra criar tarefa do terminal: `aic add "texto #repo @hoje !alta"`. **Ctrl+Shift+K** vincula uma tarefa à sessão em foco: o Claude recebe o contexto da tarefa.

## Worktrees

A aba Worktrees (**Ctrl+Shift+G**) mostra os worktrees de cada repo e quem está usando cada um. Eles ficam em `<repo>.worktrees/<branch>`, ao lado do repo. `n` cria, `Enter` abre uma sessão nele, `Delete` remove (a branch fica). Ao abrir uma tarefa com card (`DEV-1256`), a branch sai com o nome do card.

## Tokens

A aba Tokens (**Ctrl+Shift+U**) mostra os limites da conta (janela de 5h e semana), com barra verde até 50%, amarela até 80% e vermelha depois, e o uso por sessão.

## Cache ao fechar e reabrir

Workspaces, sessões e layout são salvos o tempo todo, como no tmux. Ao fechar (**Ctrl+Shift+Q**) você escolhe:

- **Salvar**: guarda o scrollback e o layout. No próximo início o Plankton pergunta *"Reabrir a última sessão?"* (`Enter` reabre com `claude --resume`, `N` começa do zero).
- **Sem salvar**: o cache some.

As tarefas nunca se perdem, só as sessões e o layout. Para começar limpo uma vez: `plankton --fresh`. Dá pra limpar o cache na aba Config.

## Notificações e som

Quando uma sessão termina, o Plankton toca o seu gemido e manda uma notificação do sistema. Quando uma sessão pede permissão, avisa também (o som de atenção é opcional). Por padrão só avisa se a janela estiver sem foco ou a sessão não estiver visível. Volume, arquivos de som e notificações ficam em **Config** (**Ctrl+,**).

## Forks

Forks são filhos do Plankton com identidade própria: tema, mascote, nomes dos estados e ajuda. Troque pela paleta (*Trocar de fork*) e alterne as variantes de tema com **Ctrl+Shift+Y**. O seletor também aparece ao abrir, se você marcar *"perguntar sempre ao abrir"*.

Variantes do Plankton:

- **balde**: o Balde de Lixo. Metal escuro, verde Plankton e o vermelho do olho. Bolhas da Fenda do Biquíni subindo no fundo.
- **karen**: o computador-esposa. Fósforo verde-água num CRT.

Quer o seu próprio Plankton animado? Coloque sprites em `~/.local/share/aic/forks/plankton/sprites/<estado>.(gif|png)` (estados: `boot`, `working`, `idle`, `waiting`, `done`, `gameover`). O que faltar cai no desenho vetorial.

Quer criar um fork? Copie `crates/aic-app/src/forks/template.rs`, ajuste e registre em `skins::all()`.

## Atalhos principais

| Atalho | Ação |
| --- | --- |
| Ctrl+Shift+N | Nova sessão Claude |
| Ctrl+Shift+T | Novo terminal (shell) |
| Ctrl+Shift+W | Fechar sessão |
| Ctrl+Shift+R | Renomear sessão |
| Ctrl+PageDown / Ctrl+PageUp | Próxima / anterior sessão |
| Ctrl+Shift+A | Ir pra próxima que precisa de você |
| Ctrl+Shift+← → ↑ ↓ | Mover o foco entre sessões |
| Alt+1 … Alt+9 | Ir pra sessão N |
| Ctrl+Shift+L | Alternar foco / grade |
| Ctrl+Shift+M | Maximizar sessão (zoom) |
| Ctrl+Shift+Esc | Soltar o teclado do terminal |
| Ctrl+Shift+Space | Nova tarefa (captura rápida) |
| Ctrl+Shift+K | Vincular tarefa à sessão |
| Ctrl+Shift+O | Trocar de workspace |
| Ctrl+Shift+PageDown / PageUp | Próximo / anterior workspace |
| Ctrl+Shift+S / B / G / U | Abas Sessões / Board / Worktrees / Tokens |
| Ctrl+, | Configurações e binds |
| F1 | Ajuda do fork |
| Ctrl+Shift+P | Paleta de comandos |
| Ctrl+Shift+= / - / 0 | Aumentar / diminuir / restaurar a fonte |
| Ctrl+Shift+Y | Próxima variante de tema |
| Ctrl+Shift+D | Mostrar/esconder a lista lateral |
| Ctrl+Shift+Q | Sair |

No terminal: **Ctrl+Shift+C / Ctrl+Shift+V** copiam e colam, **Shift+Enter** quebra linha no prompt do Claude, **Ctrl+clique** abre link.

Todos os atalhos são editáveis em **Config › Atalhos**; a lista completa (com os seus binds atuais) está logo abaixo.
