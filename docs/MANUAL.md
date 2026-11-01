# Anakin — o manual

> now this is podracing

Uma central para trabalhar com vários Claude Code ao mesmo tempo, no jeito do i3: a janela é terminal de ponta a ponta, com uma barra fina em cima. Cada contexto de trabalho (hermes, wallet…) é uma **sessão** com seus **terminais**. Por baixo, os terminais rodam num **tmux** próprio: fechar a janela não mata nada. O comando continua sendo `aic`.

O tema é Anakin Skywalker, o cavaleiro Jedi: azul do sabre, areia de Tatooine. O desenho, o boneco e o som são originais, sem imagens ou gravações dos filmes.

## Primeiros passos

1. **Ctrl+B B** abre uma sessão nova (escolha a pasta ou um worktree). **Ctrl+B c** põe mais um Claude nela e **Ctrl+B %** abre um shell ao lado, na mesma pasta.
2. **Ctrl+Tab** (ou **Alt+Tab**) anda entre as abas: Tarefas, Sessões e Tokens. **Alt+1..9** vai direto pra sessão N.
3. Esqueceu um atalho? Passe o mouse em cima do que quer: toda dica diz a tecla. **Ctrl+Space** (ou **Ctrl+B ?**) mostra todos, agrupados.

## A barra

Em cima, centralizadas, sempre as três abas: **Tarefas · Sessões · Tokens**.

Na aba **Sessões** aparece uma segunda linha, também centralizada, com as sessões numeradas e o **+**:

- a da tela fica sublinhada; a que acabou ou precisa de você fica colorida; a que está trabalhando ganha um ponto verde;
- clique direito numa sessão: renomear, arranjo, mover, fechar;
- à esquerda, os **workspaces**, só quando há mais de um;
- à direita, o **terminal em foco**: estado, nome, modelo, contexto usado, branch (e `±N` arquivos mudados) e custo.

Worktrees, Changes, Config e Ajuda abrem pelo prefixo ou pela paleta, e o nome da tela aparece na segunda linha enquanto estiver aberta. **Ctrl+B x** volta pros terminais.

## Sessões e terminais

Sessão é o que aparece na barra; terminal é cada Claude ou shell dentro dela.

- **Ctrl+B x** fecha **só o terminal em foco**: o foco vai pro vizinho e a sessão continua. Quando o último terminal fecha, a sessão some da barra, como um workspace vazio do i3.
- **Ctrl+B &** fecha a sessão inteira, com todos os terminais.
- Digitar `exit` num shell fecha o terminal. Se um Claude sair, a tela oferece reabrir.
- **Ctrl+Alt+→ / ←** (ou **Ctrl+B > / <**) vai pra próxima sessão / anterior; **Ctrl+B o / O** pro próximo terminal.
- **Ctrl+B Space** troca o arranjo: lado a lado → principal → empilhado → grade. **Ctrl+B z** maximiza o terminal.
- **Ctrl+B R** renomeia a sessão, **Ctrl+B r** o terminal. **Ctrl+B { }** move a sessão na barra.
- Clique direito no terminal: dividir, maximizar, renomear, reiniciar, fechar.

## O prefixo

Aperte **Ctrl+B**, solte e aperte a tecla. A barra acende **Ctrl+B** enquanto espera; funciona com o terminal focado.

| Tecla | Ação |
| --- | --- |
| `B` | nova sessão |
| `c` | novo Claude nesta sessão |
| `%` ou `"` | novo terminal ao lado |
| `x` | fecha o terminal em foco (numa outra tela, volta pros terminais) |
| `&` | fecha a sessão inteira |
| `h j k l` ou setas | ←/→ andam entre terminais, ↑/↓ entre sessões |
| `o` / `O` | próximo / anterior terminal |
| `n` / `p` | próxima / anterior aba (como Ctrl+Tab) |
| `>` / `<` | próxima / anterior sessão |
| `z` | maximiza / restaura o terminal |
| `a` | vai pra próxima que precisa de você |
| `Space` | muda o arranjo dos terminais |
| `r` / `R` | renomeia o terminal / a sessão |
| `{` / `}` | move a sessão na barra |
| `e` | notas do terminal em foco |
| `t` | tarefas (já escrevendo uma nova) |
| `u` | tokens |
| `s` | volta pros terminais |
| `g` `G` `,` | Worktrees, Changes, Config |
| `/` | pesquisar nas configurações |
| `1` a `9`, `(` `)`, `Tab` | workspace N, anterior/próximo, voltar pro de antes |
| `w` `W` `$` | lista, novo e renomear workspace |
| `L` / `m` | conversa com o Anakin / falar com ele por voz |
| `Y` / `A` | reiniciar o Anakin / mostrar ou esconder o boneco |
| `:` | paleta de comandos |
| `?` | catálogo de atalhos |
| `[` | solta o teclado do terminal |
| `Ctrl+B` | manda um Ctrl+B de verdade pro terminal |
| `Esc` | cancela |

O prefixo e cada tecla mudam em **Config › Atalhos** ou no `~/.config/aic/keybinds.toml`:

```toml
[prefix]
close_focused = "q"
focus_left = ["h", "←"]
```

## Tarefas

Uma lista à parte, no jeito do "Próximos" do Todoist: **data e tarefa**, agrupadas por dia. Não tem ligação nenhuma com sessões ou terminais.

**Ctrl+B t** abre com o cursor no campo. A régua do topo mostra duas semanas: clicar num dia escolhe a data. O campo da data entende `hoje`, `amanhã`, `sex`, `15/11`, `+3` (vazio = sem data), e `@sexta` no meio do texto também vale. **Enter** cria e continua no campo.

A lista mostra as atrasadas (com um botão pra mandar tudo pra hoje), os próximos 7 dias, mais tarde e sem data.

| Tecla (fora do campo) | Ação |
| --- | --- |
| `n` | escrever uma nova |
| `h` / `l` | dia anterior / seguinte da tarefa nova |
| `j` / `k` | andar na lista |
| `x` ou `Espaço` | concluir |
| `e` ou `Enter` | editar o título |
| `<` / `>` | um dia antes / depois |
| `1` `2` `3` `0` | hoje, amanhã, daqui a uma semana, sem data |
| `H` | mandar as atrasadas pra hoje |
| `d`, depois `u` | apagar, desfazer |
| `o` | abrir o link que estiver no título |
| `c` | mostrar concluídas |
| `Esc` | voltar pros terminais |

Pelo terminal: `aic add "pagar o boleto @sexta"`, `aic tasks`, `aic done <id>`.

## Notas

Cada terminal tem a sua nota em Markdown. **Ctrl+B e** abre o painel à direita já com o cursor nela; de novo fecha. Salva sozinha; **Esc** devolve o teclado pro terminal.

## Anakin (assistente)

O próprio Anakin mora no canto da tela: um boneco em pixel art, desenhado pelo app, que anda de vez em quando, acende o sabre enquanto alguém trabalha, pula quando uma sessão acaba e solta falas num balão ("hermes acabou. Yippee!"). Quando a conversa está fechada, é no balão que ele conta o que respondeu.

Clique nele (ou **Ctrl+B L**) e abre a conversa, numa janela flutuante: um Claude sempre aberto. Você pede em português, digitando ou falando, e ele mexe no app por você: cria workspaces, anota tarefas, abre sessões num repo ou worktree com a primeira mensagem já escrita. **Ctrl+B A** esconde o boneco.

> "anota pra eu lembrar: renovar a CNH sexta, comprar ração"
> "abre uma sessão com o card DEV-142 no projeto api"

- **Voz**: `Ctrl+B m` (ou o microfone na conversa) começa a gravar; de novo para, transcreve e manda. `Esc` na conversa descarta. Fora do app, `aic voz` faz o mesmo (bom pra um atalho do i3: `bindsym $mod+v exec aic voz`).
- **Microfone e transcrição**: em Config → Anakin. **whisper.cpp** local (nada sai da máquina), uma **API** no formato da OpenAI (OpenAI ou Groq; chave no `~/.config/aic/.env`) ou um **comando seu** (`{file}` e `{lang}`).
- **A conversa continua**: a sessão tem id fixo e volta com `--resume`. "Conversa nova" (Config → Anakin) começa outra.
- **Como ele opera**: pelo `aic` de linha de comando, que pede ao app aberto por uma caixa de entrada. Os mesmos comandos servem pra você ou pra scripts:

```bash
aic ws new 'Pessoal' --go              # cria workspace (e vai pra ele)
aic ws go pess                          # troca (nome ou começo)
aic add 'renovar CNH @sexta'
aic session --repo api --branch fix/juros -- 'corrija o cálculo de juros'
aic sessions | aic ws ls | aic repos    # o que está aberto
aic anakin --send 'o que tenho pra hoje?'
```

## Workspaces

Conjuntos de sessões separados, como os workspaces do i3. A barra só mostra as sessões do workspace atual; as tarefas são uma lista só. **Ctrl+B W** cria, **Ctrl+B 1..9** troca, **Ctrl+B Tab** volta pro anterior.

## Estados

O app acompanha cada Claude pelos hooks do Claude Code:

| Nome | Quando |
| --- | --- |
| IGNITING | abrindo |
| PODRACING | o Claude está trabalhando |
| MEDITATING | esperando você digitar |
| BAD FEELING | pediu permissão ou fez uma pergunta |
| YIPPEE | terminou e você ainda não olhou |
| HIGH GROUND | o processo morreu |

**Ctrl+B a** pula pra próxima que precisa de você, mesmo em outro workspace. Os hooks, a statusline (que só grava os números da barra, sem imprimir no terminal) e o plugin `aic` só valem nas sessões abertas pelo app: o seu `~/.claude` não é alterado.

## Som e notificações

Um som só: a ignição de um sabre de luz, sintetizada pelo próprio app, quando uma sessão acaba ou precisa de você. A notificação cita a sessão: **hermes acabou**, **wallet precisa de você**. Por padrão só avisa se a janela estiver sem foco ou a sessão não estiver na tela. Quer outro som? Em Config → Som, aponte pra um arquivo seu (`.mp3`, `.ogg`, `.wav`).

## Worktrees, Changes, Tokens, Config

- **Ctrl+B g**, Worktrees: ficam em `<repo>.worktrees/<branch>`. `n` cria, `Enter` abre uma sessão nele, `Delete` remove (a branch fica), `[ ]` troca de repo, `p` faz prune, `r` atualiza.
- **Ctrl+B G**, Changes: arquivos mudados e diff do repo do terminal em foco. `j/k` escolhe, `s`/`u` faz stage/unstage (`Shift+S`/`Shift+U` em todos), `d` descarta, `o` abre um shell na pasta, `f` segue o terminal em foco, `r` atualiza; o commit sai da caixa no topo.
- **Ctrl+B u**, Tokens: limites da conta, uso por sessão e histórico de custo.
- **Ctrl+B ,**, Config. **Ctrl+B /** vai direto pra busca: digite `fonte`, `som`, `prefixo`, `workspace 3`.

## Módulos

Tudo além dos terminais liga e desliga em Config → Módulos: tmux, workspaces, Anakin, tarefas, notas, worktrees, Changes, tokens, catálogo de atalhos, som e notificações.

## tmux por baixo

Os terminais rodam num servidor tmux só do app (config em `~/.local/share/aic/tmux.conf`, invisível). Fechar a janela ou o app cair não mata os Claudes: ao abrir de novo, ele reconecta. O seu tmux não é tocado.

## Fechar e reabrir

Sessões e layout são salvos o tempo todo. Ao sair (**Ctrl+Shift+Q**) você escolhe salvar ou não; no próximo início o app oferece reabrir. Para começar limpo uma vez: `aic --fresh`.

## Atalhos diretos

| Atalho | Ação |
| --- | --- |
| Ctrl+Tab / Ctrl+Shift+Tab, Alt+Tab / Alt+Shift+Tab | Próxima / anterior aba: Tarefas → Sessões → Tokens |
| Ctrl+Alt+→ / ← | Próxima / anterior sessão |
| Alt+1 … Alt+9 | Ir pra sessão N |
| Ctrl+Shift+Enter | Nova sessão |
| Ctrl+Shift+N | Novo Claude nesta sessão |
| Ctrl+Shift+T | Novo terminal ao lado |
| Ctrl+Shift+W | Fechar o terminal |
| Ctrl+Shift+X | Fechar a sessão inteira |
| Ctrl+Shift+R / F2 | Renomear o terminal / a sessão |
| Ctrl+PageDown / Ctrl+PageUp | Próximo / anterior terminal |
| Ctrl+Shift+← → ↑ ↓ | Mover o foco |
| Ctrl+Shift+A | Próxima que precisa de você |
| Ctrl+Shift+M | Maximizar o terminal |
| Ctrl+Shift+\\ | Arranjo dos terminais |
| Ctrl+Shift+E | Notas do terminal |
| Ctrl+Shift+Space | Tarefas (escrevendo uma nova) |
| Ctrl+Shift+U / G | Tokens / Worktrees |
| Ctrl+Alt+G | Changes |
| Ctrl+Shift+S | Voltar pros terminais |
| Ctrl+Shift+O | Lista de workspaces |
| Ctrl+Alt+1 … Ctrl+Alt+9 | Ir pro workspace N |
| Ctrl+Shift+PageDown / PageUp | Próximo / anterior workspace |
| Ctrl+Shift+Esc | Soltar o teclado do terminal |
| Ctrl+, | Configurações e atalhos |
| F1 | Este manual |
| Ctrl+Shift+P | Paleta de comandos |
| Ctrl+Shift+= / - / 0 | Aumentar / diminuir / restaurar a fonte |
| Ctrl+Shift+Q | Sair |

No terminal: **Ctrl+Shift+C / Ctrl+Shift+V** copiam e colam, **Shift+Enter** quebra linha no prompt do Claude, **Ctrl+clique** abre link.
