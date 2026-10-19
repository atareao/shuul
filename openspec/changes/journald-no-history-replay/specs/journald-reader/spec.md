## ADDED Requirements

### Requirement: No history replay

El lector SHALL alimentar el pipeline Jail únicamente con entradas posteriores
al inicio del intento de conexión en curso.

Para cada entrada el lector SHALL:

- descartarla si su `__REALTIME_TIMESTAMP` es anterior al instante en que se
  inició el intento de conexión;
- descartarla si `__REALTIME_TIMESTAMP` falta o no es un entero decimal; una
  entrada sin instante no se puede situar respecto al corte y se descarta por
  precaución;
- descartarla si su `__CURSOR` coincide con el último `__CURSOR` ya procesado,
  porque el stream puede reanudarse en esa misma entrada;
- alimentar el pipeline Jail en caso contrario.

El descarte SHALL ser silencioso a nivel `INFO` (a lo sumo `DEBUG`) y NO SHALL
contarse como error.

Este filtrado NO SHALL depender de la cabecera `Range`: gatewayd no garantiza
el `skip` de `entries=<cursor>:1` sin `num_entries`, ni aplica `realtime` cuando
la petición lleva `follow`.

#### Scenario: Entries older than the connection attempt are ignored
- **GIVEN** el journal contiene access logs de Traefik con fallos de días anteriores
- **WHEN** el reader arranca y abre la conexión
- **THEN** esas entradas se leen pero NO se entregan al pipeline Jail
- **AND** no se registra ningún fallo en el rate limiter por ellas
- **AND** no se crea ningún baneo a partir de ellas

#### Scenario: Live entries after startup are processed
- **GIVEN** el reader está conectado y ha fijado el corte del intento actual
- **WHEN** llega un access log con `__REALTIME_TIMESTAMP` posterior al corte
- **THEN** la entrada se entrega al pipeline Jail con normalidad

#### Scenario: Reconnect does not reprocess the last entry
- **GIVEN** el reader ya procesó una entrada y guardó su `__CURSOR`
- **WHEN** la conexión se corta y el reader reconecta
- **AND** el stream vuelve a entregar esa misma entrada
- **THEN** el lector la descarta por coincidir con el último `__CURSOR`
- **AND** el rate limiter no la cuenta dos veces

#### Scenario: Entry without a usable timestamp is discarded
- **GIVEN** el stream entrega una entrada sin `__REALTIME_TIMESTAMP`, o con un
  valor no numérico como `abc`
- **WHEN** el lector la evalúa contra el corte del intento de conexión en curso
- **THEN** la entrada NO se entrega al pipeline Jail
- **AND** el rate limiter no la cuenta ni puede banear por ella
- **AND** la siguiente entrada bien formada sí se procesa con normalidad

#### Scenario: A failed connection attempt moves the cutoff forward
- **GIVEN** gatewayd lleva 30 s caído y ha vuelto
- **WHEN** el reader reintenta
- **THEN** el corte es el instante de ese nuevo intento
- **AND** lo ocurrido durante la caída no se entrega al pipeline
