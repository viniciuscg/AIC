---
name: tarefas
description: Registra, lista e conclui tarefas no AIC pelo binário ("$AIC_BIN" add/tasks/done). Use quando surgir uma pendência fora do escopo durante o trabalho ("TODO pra depois", "anota isso", "deixa registrado", bug achado de passagem), quando pedirem para ver as tarefas abertas, ou para marcar uma tarefa como concluída.
---

# Tarefas do AIC

O binário está em `$AIC_BIN` (definido nas sessões do AIC). Fora delas, use `aic` do PATH:

```bash
PK="${AIC_BIN:-aic}"
```

## Registrar pendência (TODO pra depois)

```bash
"$PK" add 'texto da tarefa @amanha'
```

- A lista é só título e data, sem vínculo com sessões. `@hoje`, `@amanha`, `@sexta`, `@12/10`, `@+3`; sem `@`, fica sem data.
- Título curto e acionável, dizendo onde está o problema (arquivo, endpoint). Uma pendência por chamada.
- Use aspas simples (o `!` e o `$` não são expandidos pelo shell).

Se o usuário pediu ("anota isso"), registre direto. Se foi você que achou a pendência, registre e avise numa linha com o texto usado.

## Listar

```bash
"$PK" tasks          # abertas
"$PK" tasks --all    # inclui concluídas
"$PK" tasks --json   # para filtrar com jq
```

## Concluir

```bash
"$PK" done <id>      # 4+ caracteres do id mostrado em `tasks`
```

Se der "mais de uma tarefa", use mais caracteres do id.
