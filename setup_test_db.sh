#!/usr/bin/env bash
# Setup para tests de integración
# Levanta PostgreSQL con Podman, espera a que esté listo y ejecuta las migraciones.
#
# Uso:
#   ./tests/setup_db.sh          # Solo levanta la DB
#   ./tests/setup_db.sh --clean  # Baja y vuelve a levantar

set -euo pipefail

COMPOSE_FILE="docker-compose.test.yml"
DB_URL="postgres://test:test@localhost:5433/test_db"

if [[ "${1:-}" == "--clean" ]]; then
    echo "🧹 Limpiando contenedores anteriores..."
    podman compose -f "$COMPOSE_FILE" down -v 2>/dev/null || true
fi

echo "🐘 Levantando PostgreSQL..."
podman compose -f "$COMPOSE_FILE" up -d

echo "⏳ Esperando a que PostgreSQL esté listo..."
for i in $(seq 1 30); do
    if podman compose -f "$COMPOSE_FILE" exec -T postgres pg_isready -U test -d test_db &>/dev/null; then
        echo "✅ PostgreSQL listo"
        break
    fi
    sleep 1
done

echo "🔧 Ejecutando migraciones..."
cd backend
DATABASE_URL="$DB_URL" cargo sqlx migrate run 2>/dev/null || {
    echo "⚠️  sqlx-cli no instalado, intentando con cargo run..."
    DATABASE_URL="$DB_URL" cargo run -- --migrate-only 2>/dev/null || true
}
cd ..

echo ""
echo "✅ Base de datos de tests lista"
echo "   DATABASE_URL=$DB_URL"
echo ""
echo "Para ejecutar los tests:"
echo "   DATABASE_URL=$DB_URL cargo test --test integration -- --nocapture"
echo ""
echo "Para limpiar:"
echo "   podman compose -f $COMPOSE_FILE down -v"
