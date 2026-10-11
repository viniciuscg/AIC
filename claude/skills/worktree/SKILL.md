---
name: worktree
description: Abre e fecha um git worktree por card, no padrão <repo>.worktrees/<branch> com branch DEV-123-slug, a partir da branch padrão do repo (main/master). Use ao começar a codar um card num repo, quando pedirem "abre um worktree", "cria a branch do card", "trabalha isolado", ou ao terminar ("fecha o worktree", "limpa o worktree").
---

# Worktree por card

Padrão (o mesmo do app AIC):

- **Branch**: `<CARD>-<slug>` — `DEV-142` + "Corrigir cálculo de juros" → `DEV-142-corrigir-calculo-de-juros`. Slug minúsculo, sem acento, palavras unidas por `-`, até ~40 caracteres. Sem card, só o slug.
- **Pasta**: ao lado do repo, `<repo>.worktrees/<branch>` (`/` da branch vira `-`). Ex: `~/dev/api-pagamentos.worktrees/DEV-142-corrigir-calculo-de-juros`.
- **Base**: a branch padrão do origin: `git -C "$REPO" symbolic-ref --short refs/remotes/origin/HEAD` (sem origin: `main`, ou `master` se for a que existe). Se o `CLAUDE.md` do projeto disser outra regra, ela vale.

## Abrir

```bash
REPO=/caminho/do/repo            # raiz do repo (não um worktree)
BRANCH=DEV-142-corrigir-calculo-de-juros
BASE=main                        # ou master (veja a regra acima)
WT="$REPO.worktrees/${BRANCH//\//-}"

git -C "$REPO" fetch origin "$BASE"
if [ -d "$WT" ]; then
  echo "já existe: $WT"
elif git -C "$REPO" rev-parse --verify --quiet "refs/heads/$BRANCH" >/dev/null; then
  git -C "$REPO" worktree add "$WT" "$BRANCH"
else
  git -C "$REPO" worktree add -b "$BRANCH" "$WT" "origin/$BASE"
fi
```

Depois trabalhe dentro de `$WT` (caminhos absolutos). Dependências (`vendor/`, `node_modules/`) não vêm junto: instale no worktree se for rodar testes.

## Fechar

1. `git -C "$WT" status` — se houver mudança não commitada, pergunte antes de descartar.
2. `git -C "$REPO" worktree remove "$WT"` (`--force` só com confirmação do usuário).
3. `git -C "$REPO" worktree prune`. A branch continua existindo; apague só se o usuário pedir.

Nunca faça push nem abra PR sem o usuário pedir.
