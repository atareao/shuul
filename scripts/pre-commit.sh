#!/usr/bin/env bash
# pre-commit.sh — plantilla para hooks de git
# Se instala vía: just setup-hooks
# Copiar a .git/hooks/pre-commit

set -euo pipefail

# Skip check en merge commits
if [ "$(git rev-parse -q --verify MERGE_HEAD 2>/dev/null)" ]; then
    exit 0
fi

# Ejecutar check-spec desde la raíz del repo
REPO_ROOT=$(git rev-parse --show-toplevel)
cd "$REPO_ROOT"

if ! just check-spec 2>/dev/null; then
    echo ""
    echo "⛔ COMMIT BLOQUEADO: No hay un change proposal aprobado."
    echo "   Crea uno con: openspec change <feature>"
    echo "   O salta este hook con: git commit --no-verify"
    exit 1
fi