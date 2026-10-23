-- Normaliza a 0 el peso de las reglas de Jail ya almacenadas.
--
-- Por qué: el pipeline de Jail evalúa TODAS las reglas que casan, sin `break`, así que
-- su orden —y por tanto su peso— no decide nada. El catálogo repartía a sus plantillas
-- pesos del mismo rango que a las del WAF (10, 50, 100, 200, 9997, 9998, 9999) y la
-- interfaz los escondía, de modo que al ordenar la tabla por Weight las filas de Jail se
-- intercalaban por números invisibles y la tabla parecía desordenada. El WAF, en cambio,
-- es `first match wins` por `weight ASC`: ahí el número SÍ decide, así que sus reglas no
-- se tocan.
--
-- El filtro `weight <> 0` deja fuera las filas que ya están bien, así que reaplicar la
-- migración no escribe nada (idempotente).
UPDATE rules
SET weight = 0
WHERE pipeline = 'jail'
  AND weight <> 0;
