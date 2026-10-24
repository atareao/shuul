-- Deja canónica la etiqueta (`jail`) y normaliza a 0 el peso de las reglas de Jail ya
-- almacenadas.
--
-- Por qué: el pipeline de Jail evalúa TODAS las reglas que casan, sin `break`, así que
-- su orden —y por tanto su peso— no decide nada. El catálogo repartía a sus plantillas
-- pesos del mismo rango que a las del WAF (10, 50, 100, 200, 9997, 9998, 9999) y la
-- interfaz los escondía, de modo que al ordenar la tabla por Weight las filas de Jail se
-- intercalaban por números invisibles y la tabla parecía desordenada. El WAF, en cambio,
-- es `first match wins` por `weight ASC`: ahí el número SÍ decide, así que sus reglas no
-- se tocan.
--
-- `COLLATE NOCASE` alcanza también el legado almacenado con otra capitalización
-- (`JAIL`, `Jail`): antes de que la API validase el pipeline, cualquier cadena cabía en
-- la columna y esas filas son tan de Jail como las que traen `jail` en minúsculas. La
-- comparación insensible a mayúsculas se aplica SOLO al literal `jail`; ninguna fila del
-- WAF casa con él, así que sus pesos quedan intactos.
--
-- Por qué se normaliza ADEMÁS la etiqueta: `shuul.rs` salta las reglas de Jail
-- comparando el literal exacto `jail`. Una fila con `JAIL` o `Jail` se quedaría con
-- peso 0 —bien— pero el WAF la seguiría evaluando, porque su comparación no reconoce
-- otra capitalización, y podría producir una decisión de allow/deny que no le
-- corresponde. La etiqueta y el peso se dejan canónicos a la vez.
--
-- Por qué sigue siendo UNA sola sentencia: el test mide con `SELECT changes()`, que
-- devuelve el recuento de la **última** sentencia de escritura de la conexión. Partir
-- el UPDATE en dos haría que la aserción de «exactamente las 6 filas de Jail» midiera
-- la sentencia equivocada.
--
-- El `AND (pipeline <> 'jail' OR weight <> 0)` es lo que la mantiene idempotente: en la
-- segunda aplicación ninguna fila matchea (etiqueta ya canónica y peso ya 0) y no se
-- escribe nada.
--
-- NO se toca la etiqueta de las filas del WAF: es un defecto preexistente y ajeno a
-- esta invariante —una fila `WAF` no la salta el pipeline de Jail—, así que queda
-- anotado como no-objetivo.
--
-- No se toca `updated_at` a propósito: esta migración normaliza un valor derivado que el
-- cliente nunca eligió de verdad —la interfaz ocultaba el campo—, no una edición del
-- usuario. Escribir la fecha falsearía «la última vez que se editó esta regla» en la
-- tabla, que es lo que ese campo significa.
UPDATE rules
SET pipeline = 'jail',
    weight = 0
WHERE pipeline = 'jail' COLLATE NOCASE
  AND (pipeline <> 'jail' OR weight <> 0);
