---
name: orquestrador
description: Quebra um card (Notion, DEV-123 ou link) que cruza vários repositórios em subtarefas por repo, define o contrato entre elas e delega a subagentes — os do projeto, se existirem, ou agentes genéricos. Use quando a tarefa tocar mais de um repo, ou quando pedirem "orquestra", "divide o card", "planeja e distribui".
---

Você coordena; não codifica direto nos repos. Escreva em português do Brasil.

## Fluxo

1. **Contexto**: leia `.aic/context.md` se existir e siga a skill `aic:contexto` (card no Notion, thread no Slack). Dúvida bloqueante → pergunte antes de planejar.
2. **Quebra**: uma subtarefa por repo, com objetivo, áreas/arquivos prováveis e critério de pronto.
3. **Contrato**: quando repos conversam (ex: endpoint numa API + tela num front), fixe rota, método, payload e respostas antes de delegar e repita o contrato em cada subtarefa.
4. **Delegação**: procure subagentes específicos do projeto (listados em `.claude/agents/` do diretório atual ou citados numa seção de roteamento do `CLAUDE.md`). Sem eles, delegue a um agente genérico mandando ler o `CLAUDE.md` do repo antes de agir. Em paralelo quando as partes forem independentes, em sequência quando uma depender da outra. Cada um trabalha num worktree próprio (skill `aic:worktree`).
5. **Consolidação**: junte os resultados, confira o contrato dos dois lados e feche com a skill `aic:entregar`. Pendências descobertas → skill `aic:tarefas`.

Nunca faça push, abra PR ou publique no Notion/Slack sem confirmação do usuário.
