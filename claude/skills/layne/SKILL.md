---
name: layne
description: Comandos para operar o AIC pela linha de comando — criar/trocar workspaces, anotar e concluir tarefas, abrir sessões do Claude (ou shell) num repo ou worktree com uma primeira mensagem, abrir um card do Notion montando o bloco, listar o que está aberto. Use quando o usuário (normalmente pelo Layne, por texto ou voz) pedir para organizar o trabalho no AIC: "cria um workspace", "anota pra eu lembrar", "abre uma sessão com o card X no projeto Y", "o que tenho aberto?".
---

# Operar o AIC (skill do Layne)

Tudo pelo binário `aic` (no PATH). Os comandos que mudam a tela (`ws new/go`, `session`, `card`) pedem ao app aberto e esperam a resposta: a linha `✓ …` confirma; erro sai com código ≠ 0 e a mensagem diz o porquê. Use aspas simples nos textos (`!` e `$` não expandem).

## Ver o estado

```bash
aic ws ls                 # workspaces, nº de terminais e de tarefas abertas
aic sessions              # terminais abertos por workspace (id curto, tipo, nome, pasta)
aic tasks [--all]         # tarefas abertas (id curto, status, prioridade, título, marcadores)
aic repos                 # repos conhecidos (nome → pasta), para --repo e #repo
```

Todos aceitam `--json` exceto `repos`. Olhe o estado antes de criar algo que talvez já exista.

## Workspaces

```bash
aic ws new 'Pessoal' --go    # cria (e vai pra ele com --go); se já existe, só avisa
aic ws go pess               # vai pro workspace (nome exato ou começo do nome)
```

## Tarefas

```bash
aic add --ws 'Pessoal' 'comprar presente da Ana @sexta !alta'
aic done 3f9a            # 4+ caracteres do id
```

- Marcadores no texto: `@hoje` `@amanha` `@sexta` `@12/10` (prazo), `!alta` `!baixa` (prioridade), `#repo`, `^DEV-142` (card), `~tag`. `+workspace` também funciona para nomes de uma palavra; prefira `--ws`.
- Uma tarefa por chamada; título curto e acionável. Não use `--session` (isso vincula à sua própria sessão).
- O workspace precisa existir: crie com `aic ws new` antes.

## Sessões (terminais)

```bash
aic session --repo api-pagamentos -- 'Corrija o cálculo de juros em boleto (card DEV-142). Leia o CLAUDE.md antes.'
aic session --repo api --branch fix/juros --ws 'Sprint 42' --name 'juros'   # num worktree novo
aic session --cwd ~/notas --shell                                          # um shell
aic session --repo api --task 3f9a                                         # vincula a tarefa (contexto vai pro Claude)
```

- Abre num bloco novo, no workspace atual ou no `--ws` (criado se não existir), e foca nele.
- `--repo` aceita o nome ou o começo dele (`aic repos`). `--branch` cria um worktree `<repo>.worktrees/<branch>`.
- Tudo depois de `--` é a primeira mensagem do Claude: escreva-a completa (objetivo, card, onde mexer, critério de pronto), porque a sessão nova não vê esta conversa.

## Abrir card (Notion)

```bash
aic card 'https://www.notion.so/…' --auto              # busca o card e monta o bloco com os repos sugeridos
aic card DEV-142 --auto --repos api,front --ws 'Sprint 42'
aic card DEV-142                                        # só abre o diálogo no app (o usuário escolhe os repos)
```

O `--auto` leva até um minuto ou dois (o AIC lê o card pelo Claude). Monta: worktree por repo na branch do card, um Claude em cada com o contexto do card, notas do bloco. Se o usuário citou os projetos, passe `--repos`; senão, deixe os sugeridos e diga quais foram.

## Receitas

- **"Cria um workspace pessoal e abre umas tarefas pra eu lembrar: X, Y e Z"** → `aic ws new 'Pessoal' --go` e um `aic add --ws 'Pessoal' '…'` por item. Responda com a lista e os ids.
- **"Abre uma sessão com o card DEV-142 no projeto api"** → `aic card DEV-142 --auto --repos api`. Se for só uma conversa rápida sem card, `aic session --repo api -- '…'`.
- **"O que tenho pra hoje?"** → `aic tasks --json` e filtre por prazo hoje/atrasado; responda em lista curta.
- **"Terminei a tarefa do presente"** → `aic tasks`, ache pelo título, `aic done <id>`. Mais de uma candidata → pergunte qual.

Se `aic` responder que o AIC não está aberto, diga isso ao usuário; não tente abrir a interface.
