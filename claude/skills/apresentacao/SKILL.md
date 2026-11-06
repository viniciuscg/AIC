---
name: apresentacao
description: Monta o material da apresentação de engenharia do usuário — o que ele fez, em que atuou, que dor atacou e como resolveu — cruzando tarefas do AIC, páginas do Notion (post mortems, incidentes), mensagens dele no Slack (post_mortem, alertas, incidentes) e PRs do GitHub. Só lê; nada é publicado. Use quando pedirem "monta minha apresentação", "o que eu fiz essa semana/sprint", "resumo pra apresentação de engenharia", "levanta meus post mortems e PRs".
---

# Apresentação de engenharia

Objetivo: um roteiro do que o usuário fez no período, contado como **dor → atuação → solução → evidência**, pronto para ele apresentar.

## Regras de privacidade (não negociáveis)

- **Somente leitura.** Não crie, edite nem comente nada no Notion, não mande mensagem nem reação no Slack, não comente em PR. Nem rascunho.
- Notion e Slack **só pelas ferramentas MCP** da sessão (`notion-*`, `slack_*`). Nada de WebFetch, WebSearch, curl ou outro serviço com conteúdo interno.
- GitHub: não há MCP; use o `gh` local, só com comandos de leitura (`gh search`, `gh pr view`, `gh api` com GET).
- O resultado fica só na máquina: `~/claude-out/apresentacao-<AAAA-MM-DD>.md`. Publicar em qualquer lugar só se o usuário pedir explicitamente, e confirmando o destino.
- No texto final, não copie segredos, tokens, URLs com credencial, nem dados de clientes (nome, CPF, e-mail, valores de pessoas). Descreva de forma genérica ("cliente do consignado", "~120 propostas").
- Se uma fonte falhar (MCP desconectado, sem login), diga qual ficou de fora e siga com o resto. Não invente.

## 1. Período

Use o que o usuário disser ("essa semana", "desde a última apresentação", "outubro"). Sem indicação, use os últimos 14 dias e diga isso no topo. Calcule `DESDE` (AAAA-MM-DD).

## 2. Coletar (rode as fontes em paralelo)

**Tarefas do AIC**
```bash
aic tasks --all --json
```
Pegue as concluídas com `done_at >= DESDE` e as abertas com `due`/`updated_at` no período ou tags como `bloqueado`. Título, `notes` e `tags` dão o tema; links de PR/Notion no título viram evidência.

**Notion (MCP)**
- `notion-get-self` para saber quem é o usuário.
- `notion-search` por post mortem, incidente, RCA, postmortem, alerta e pelos temas que apareceram nas tarefas; filtre pelo período e pelas páginas que o usuário criou ou editou.
- `notion-fetch` nas relevantes: causa, impacto, ações e responsáveis. Registre o papel dele (autor, responsável por ação, citado).

**Slack (MCP)**
- `slack_search_channels` para achar os canais de post mortem, alertas e incidentes (nomes com `post_mortem`, `postmortem`, `alerta`, `alert`, `incidente`, `incident`, `sre`, `oncall`).
- `slack_search_public_and_private` com `from:<@me> after:DESDE` em cada canal e também sem filtro de canal, com termos como "resolvido", "corrigido", "deploy", "hotfix", "rollback".
- `slack_read_thread` nas threads em que ele atuou, para entender o problema e o desfecho.

**PRs (gh, leitura)**
```bash
gh search prs --author @me --owner emcash-team --updated ">=DESDE" --limit 100 --json title,url,repository,state,closedAt,createdAt,isDraft
gh search prs --reviewed-by @me --owner emcash-team --updated ">=DESDE" --limit 100 --json title,url,repository,state
```
Para as PRs mais importantes, `gh pr view <url> --json title,body,mergedAt,additions,deletions,files` para o resumo da mudança.

## 3. Cruzar

Junte num **mesmo item** o que fala do mesmo assunto: o alerta no Slack, o post mortem no Notion, a PR do fix e a tarefa no AIC são uma história só. Use repo, card (SPK-/DEV-), nome do serviço e datas para casar. O que sobrar sozinho vira item próprio.

Para cada item, classifique o papel dele: **resolveu**, **atuou** (investigou, mitigou, ajudou), **revisou** ou **em andamento**.

## 4. Escrever

Salve em `~/claude-out/apresentacao-<hoje>.md`:

```markdown
# Apresentação de engenharia: <período>

> Fontes: AIC (n tarefas), Notion (n páginas), Slack (n threads), GitHub (n PRs). Fora: <fonte que falhou, se houver>.

## Destaques
3 a 5 bullets: o que mais importa contar.

## Incidentes e post mortems
### <título curto>
- **Dor:** o que quebrou, quem foi afetado, por quanto tempo.
- **O que eu fiz:** investigação, mitigação, comunicação.
- **Solução:** a correção e a prevenção (alerta, teste, processo).
- **Evidência:** links da PR, do post mortem e da thread.

## Entregas
O mesmo formato para features, melhorias e refactors (PRs mergeadas e tarefas concluídas).

## Reviews e apoio ao time
Lista curta: PR, repo, de quem.

## Em andamento / próximos passos
O que está aberto ou bloqueado e o que falta.
```

Tom: primeira pessoa, frases curtas, foco no impacto ("reduziu X", "parou de perder Y"). Números só se vierem das fontes.

## 5. Responder

No chat: os destaques em 3 a 5 linhas, o caminho do arquivo e as fontes que ficaram de fora. Pergunte se ele quer ajustar o período ou cortar/expandir algum item.
