-- El conjunto ordenado de condiciones de cada regla (F2b-1).
--
-- Hasta ahora el matcheo proyectaba los 14 campos de `rules` a un `Vec<Condition>`
-- en memoria (`models::condition::project`). Esta tabla pasa a ser la fuente
-- persistida de ese conjunto: una fila por condición, con `position` fijando el
-- orden de evaluación dentro de la regla. Las 14 columnas de `rules` se conservan
-- como la representación simple —siguen siendo la entrada y la salida de la API y
-- el contenido de export/import—, y cada escritura de regla proyecta esos campos
-- a esta tabla.
--
-- El vocabulario de colecciones es el de los 14 campos actuales. En esta
-- sub-entrega `key` es siempre nulo, `operator` siempre `@rx` y `negate` siempre 0:
-- son columnas adelantadas al modelo que el matcher todavía no usa, y no declararlas
-- ahora obligaría a migrar la tabla en cuanto se implemente la primera condición
-- que las necesite.
--
-- `transformations` guarda la especificación `t:` que la condición aplica al valor
-- de la petición antes de compararlo. En F2b-1 todas las condiciones de una regla
-- comparten la pipeline de la regla, así que el backfill la copia tal cual de
-- `rules.transformations`.
--
-- `ON DELETE CASCADE` hace que borrar una regla se lleve sus condiciones, de modo
-- que `Rule::delete` no necesita tocar esta tabla. `UNIQUE(rule_id, position)`
-- impide un conjunto con dos condiciones en la misma posición, que dejaría el orden
-- de evaluación sin definir.
CREATE TABLE rule_conditions (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    rule_id         INTEGER NOT NULL REFERENCES rules(id) ON DELETE CASCADE,
    position        INTEGER NOT NULL,
    collection      TEXT    NOT NULL,
    key             TEXT,
    transformations TEXT    NOT NULL DEFAULT '',
    operator        TEXT    NOT NULL DEFAULT '@rx',
    pattern         TEXT    NOT NULL,
    negate          INTEGER NOT NULL DEFAULT 0,
    UNIQUE(rule_id, position)
);

-- Backfill: una fila por cada campo de los 14 que la regla restrinja con un patrón
-- no vacío, en el orden histórico de la cadena `&&` (el de `Collection::ALL`).
--
-- La subconsulta emite una fila por campo restringido, etiquetada con `ord` = su
-- índice en `Collection::ALL`; el `ROW_NUMBER() OVER (PARTITION BY rule_id ORDER BY
-- ord)` de fuera le da la `position` contigua 0-based. Una regla sin ningún campo
-- restringido no emite filas y, por tanto, no recibe ninguna condición.
INSERT INTO rule_conditions (rule_id, position, collection, transformations, operator, pattern)
SELECT rule_id,
       ROW_NUMBER() OVER (PARTITION BY rule_id ORDER BY ord) - 1,
       collection,
       transformations,
       '@rx',
       pattern
FROM (
    SELECT id AS rule_id, transformations, 0 AS ord, 'ip_address' AS collection, ip_address AS pattern FROM rules WHERE ip_address IS NOT NULL AND ip_address <> ''
    UNION ALL
    SELECT id, transformations, 1, 'protocol', protocol FROM rules WHERE protocol IS NOT NULL AND protocol <> ''
    UNION ALL
    SELECT id, transformations, 2, 'fqdn', fqdn FROM rules WHERE fqdn IS NOT NULL AND fqdn <> ''
    UNION ALL
    SELECT id, transformations, 3, 'path', path FROM rules WHERE path IS NOT NULL AND path <> ''
    UNION ALL
    SELECT id, transformations, 4, 'query', query FROM rules WHERE query IS NOT NULL AND query <> ''
    UNION ALL
    SELECT id, transformations, 5, 'city_name', city_name FROM rules WHERE city_name IS NOT NULL AND city_name <> ''
    UNION ALL
    SELECT id, transformations, 6, 'country_name', country_name FROM rules WHERE country_name IS NOT NULL AND country_name <> ''
    UNION ALL
    SELECT id, transformations, 7, 'country_code', country_code FROM rules WHERE country_code IS NOT NULL AND country_code <> ''
    UNION ALL
    SELECT id, transformations, 8, 'user_agent', user_agent FROM rules WHERE user_agent IS NOT NULL AND user_agent <> ''
    UNION ALL
    SELECT id, transformations, 9, 'method', method FROM rules WHERE method IS NOT NULL AND method <> ''
    UNION ALL
    SELECT id, transformations, 10, 'referer', referer FROM rules WHERE referer IS NOT NULL AND referer <> ''
    UNION ALL
    SELECT id, transformations, 11, 'content_type', content_type FROM rules WHERE content_type IS NOT NULL AND content_type <> ''
    UNION ALL
    SELECT id, transformations, 12, 'accept_language', accept_language FROM rules WHERE accept_language IS NOT NULL AND accept_language <> ''
    UNION ALL
    SELECT id, transformations, 13, 'x_request_id', x_request_id FROM rules WHERE x_request_id IS NOT NULL AND x_request_id <> ''
);
