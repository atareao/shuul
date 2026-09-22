#!/usr/bin/env bash
# check-spec.sh — Verifica que existe un change proposal aprobado antes de editar código.
# Exit 0 si hay spec aprobado, exit 1 si no.

set -euo pipefail

CHANGES_DIR="openspec/changes"

# Si no existe el directorio de changes, no hay spec
if [ ! -d "$CHANGES_DIR" ]; then
    echo "❌ No OpenSpec change proposal found."
    echo "   Run 'openspec change <feature>' to create one."
    exit 1
fi

# Buscar cualquier change proposal con tasks.md (no archivado)
found=false
for proposal in "$CHANGES_DIR"/*/; do
    if [ -f "${proposal}tasks.md" ]; then
        found=true
        break
    fi
done

if [ "$found" = false ]; then
    echo "❌ No active change proposal found (no tasks.md in any proposal)."
    echo "   Run 'openspec change <feature>' to create one."
    exit 1
fi

echo "✅ Active change proposal found."
exit 0
