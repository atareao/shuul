-- Backfill de la colección booleana `is_tor` a `rule_conditions` (F2b-3).
--
-- `rule_conditions` es la fuente **única** del matcheo: la carga lee la tabla
-- (`Rule::read_conditions`) y la columna `is_tor` de `rules` no se reinterpreta en
-- caliente. Sin este backfill, una regla con `is_tor = 1` almacenada antes de
-- esta sub-entrega perdería su condición Tor al arrancar.
--
-- Para cada regla con `is_tor = 1` que aún **no** tenga su fila `is_tor`, inserta
-- una fila `collection='is_tor'`, `key=NULL`, `transformations=''`,
-- `operator='@truthy'`, `pattern=''`, `negate=0`, con la `position` **contigua**
-- tras las filas existentes de esa regla (la proyección de los 14 campos ya las
-- dejó en `20261025000000_create_rule_conditions.up.sql`).
--
-- El `NOT EXISTS` la hace **idempotente**: reaplicarla no duplica la fila.
INSERT INTO rule_conditions (rule_id, position, collection, key, transformations, operator, pattern, negate)
SELECT r.id,
       COALESCE(MAX(rc.position), -1) + 1,
       'is_tor',
       NULL,
       '',
       '@truthy',
       '',
       0
FROM rules r
LEFT JOIN rule_conditions rc ON rc.rule_id = r.id
WHERE r.is_tor = 1
  AND NOT EXISTS (
      SELECT 1 FROM rule_conditions existing
      WHERE existing.rule_id = r.id AND existing.collection = 'is_tor'
  )
GROUP BY r.id;
