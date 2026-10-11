---
name: entregar
description: Fecha uma tarefa do AIC — resume o que mudou, roda o checklist (testes, lint, formatação), sugere comentário para o card do Notion e a thread do Slack (só publica com confirmação) e conclui com aic done. Use quando o trabalho do card estiver pronto, ou quando pedirem "entrega", "fecha a tarefa", "finaliza", "terminei".
---

# Entregar a tarefa

## 1. O que mudou

- `git status` e `git diff <base>...HEAD --stat` (base `master` nos repos PHP, `main` no resto) em cada repo/worktree tocado.
- Resuma em 3–6 itens: comportamento novo/corrigido, arquivos-chave, migrations, variáveis de ambiente novas.

## 2. Checklist

Rode o que o repo tiver (veja `CLAUDE.md`, `Makefile`, `composer.json`, `package.json` ou `go.mod`) e mostre o resultado de cada item:

- [ ] Testes do que mudou (e a suíte, se for rápida).
- [ ] Lint / análise estática.
- [ ] Formatação.
- [ ] Sem segredo, debug ou arquivo temporário no diff (`.aic/` fica fora do git).
- [ ] Commits no padrão do repo.

Se algo falhar, pare e mostre — não declare entregue.

## 3. Comentário no card e na thread

Escreva um rascunho curto (o que mudou, como testar, branch/PR). Mostre ao usuário e **só publique com confirmação explícita**:

- Notion: comentário no card pela ferramenta MCP do Notion (ex: `notion-create-comment`).
- Slack: resposta na thread pela ferramenta MCP do Slack (ex: `slack_send_message` com o `thread_ts`).

## 4. Concluir no AIC

```bash
PK="${AIC_BIN:-aic}"
"$PK" tasks                     # ache o id (a tarefa da sessão está em $AIC_TASK)
"$PK" done "${AIC_TASK:-<id>}"
```

Pendências que sobraram → registre com a skill `aic:tarefas` antes de concluir. Se houver worktree aberto e o trabalho já estiver mergeado, ofereça fechá-lo (skill `aic:worktree`).
