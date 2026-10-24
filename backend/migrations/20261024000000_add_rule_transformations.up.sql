-- La especificación de transformaciones de la regla (idea de Coraza): una
-- pipeline `t:` que se aplica al valor de la petición antes de compararlo con
-- el regex. El valor por defecto es la cadena vacía porque la cadena vacía ES
-- la pipeline vacía: el valor no se toca y ninguna regla existente cambia de
-- comportamiento. Un valor por defecto no vacío habría alterado el matcheo de
-- todas las filas ya almacenadas al aplicar la migración.
ALTER TABLE rules ADD COLUMN transformations TEXT NOT NULL DEFAULT '';
