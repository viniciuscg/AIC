---
name: contexto
description: Reúne o contexto da tarefa antes de codar — lê .aic/context.md, busca o card no Notion e a thread do Slack pelas ferramentas MCP e resume requisitos. Use no início de uma sessão aberta pelo AIC com tarefa vinculada, quando o prompt citar um card (DEV-123) ou um link do Notion/Slack, ou quando pedirem "pega o contexto", "lê o card", "o que essa tarefa pede".
---

# Contexto da tarefa

Objetivo: entender o pedido antes de mexer em código. Não escreva código nesta etapa.

## Passos

1. **Arquivo local**: se existir `.aic/context.md` no diretório atual, leia. Ele traz título, card (`DEV-123`), links do Notion e do Slack, repo, notas e subtarefas. O prompt de sistema da sessão também pode trazer esses dados ("Tarefa vinculada: …").
2. **Card no Notion**: use as ferramentas MCP do Notion disponíveis na sessão (nomes com `notion`, ex: `notion-fetch`, `notion-search`).
   - Com URL → busque a página direto (fetch).
   - Só com a chave (`DEV-123`) → pesquise pela chave e confirme que o título bate.
   - Leia descrição, critérios de aceite, comentários e páginas filhas relevantes.
3. **Thread do Slack**: com as ferramentas MCP do Slack (ex: `slack_read_thread`), leia a thread do link. `https://<ws>.slack.com/archives/<CANAL>/p<TS>` vira canal `<CANAL>` e ts com ponto antes dos 6 últimos dígitos (`p1712345678123456` → `1712345678.123456`).
4. Se uma ferramenta MCP não existir ou falhar (sem login), diga qual faltou e siga com o que tem. Não invente conteúdo do card.

## Saída

Resumo curto, em português:

- **Objetivo**: uma frase.
- **Requisitos / critérios de aceite**: lista.
- **Decisões e restrições** vindas da thread (quem decidiu, quando).
- **Dúvidas em aberto**: o que está ambíguo — pergunte ao usuário antes de assumir.
- **Repos afetados** e próximo passo (ex: abrir worktree com a skill `aic:worktree`; card que cruza repos → agente `aic:orquestrador`).

Pendências fora do escopo que aparecerem → registre com a skill `aic:tarefas`.
