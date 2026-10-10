---
description: Cria uma tarefa no AIC vinculada a esta sessão (ex — /aic:tarefa revisar juros #api !alta @amanha)
argument-hint: texto #repo @quando !prioridade ^DEV-123 ~tag
allowed-tools: Bash
---

Crie uma tarefa no AIC com o texto abaixo, vinculada à sessão atual.

Texto: $ARGUMENTS

Rode com a ferramenta Bash, passando o texto como um único argumento entre aspas simples (aspa simples interna vira `'\''`):

```bash
"${AIC_BIN:-aic}" add --session '<texto>'
```

Se o texto estiver vazio, pergunte o que registrar em vez de rodar. Mostre a saída do comando sem comentários extras. Se falhar (ex: binário não encontrado), mostre o erro e sugira `aic --version (e o install.sh do AIC)`.
