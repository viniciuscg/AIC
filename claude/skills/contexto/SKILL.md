---
name: contexto
description: Reúne o contexto do card antes de codar — busca o card no Notion e a thread do Slack pelas ferramentas MCP e resume requisitos. Use quando o prompt citar um card (DEV-123) ou um link do Notion/Slack, ou quando pedirem "pega o contexto", "lê o card", "o que esse card pede".
---

# Contexto do card

Objetivo: entender o pedido antes de mexer em código. Não escreva código nesta etapa.

## Passos

1. **Card no Notion**: use as ferramentas MCP do Notion disponíveis na sessão (nomes com `notion`, ex: `notion-fetch`, `notion-search`).
   - Com URL → busque a página direto (fetch).
   - Só com a chave (`DEV-123`) → pesquise pela chave e confirme que o título bate.
   - Leia descrição, critérios de aceite, comentários e páginas filhas relevantes.
2. **Thread do Slack**: com as ferramentas MCP do Slack (ex: `slack_read_thread`), leia a thread do link. `https://<ws>.slack.com/archives/<CANAL>/p<TS>` vira canal `<CANAL>` e ts com ponto antes dos 6 últimos dígitos (`p1712345678123456` → `1712345678.123456`).
3. Se uma ferramenta MCP não existir ou falhar (sem login), diga qual faltou e siga com o que tem. Não invente conteúdo do card.

## Saída

Resumo curto, em português:

- **Objetivo**: uma frase.
- **Requisitos / critérios de aceite**: lista.
- **Decisões e restrições** vindas da thread (quem decidiu, quando).
- **Dúvidas em aberto**: o que está ambíguo — pergunte ao usuário antes de assumir.
- **Repos afetados** e próximo passo (ex: abrir worktree com a skill `aic:worktree`; card que cruza repos → agente `aic:orquestrador`).

Pendências fora do escopo que aparecerem → registre com a skill `aic:tarefas`.
