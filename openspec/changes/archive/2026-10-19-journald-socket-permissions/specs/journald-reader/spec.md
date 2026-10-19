## ADDED Requirements

### Requirement: Configuration diagnostics

Cuando la configuración del transporte no sea utilizable, el lector SHALL emitir un
diagnóstico que identifique el valor rechazado y, cuando exista una corrección evidente,
nombre la variable que lo aceptaría.

El diagnóstico NO SHALL romper el fail-open: el fallo se registra como warning y se
reintenta cada `retry_delay`; el proceso y el pipeline WAF no se ven afectados.

#### Scenario: Absolute path passed as the gateway URL
- **GIVEN** `SHUUL_JOURNALD_URL=/run/systemd/journal/gatewayd.sock`
- **WHEN** el lector construye el transporte a partir del entorno
- **THEN** el error incluye el valor recibido
- **AND** menciona `SHUUL_JOURNALD_SOCKET` como la variable que acepta ese valor
- **AND** se registra como warning y se reintenta, sin abortar el arranque

#### Scenario: URL without a scheme that is not a path
- **GIVEN** `SHUUL_JOURNALD_URL=localhost:19531`
- **WHEN** el lector construye el transporte a partir del entorno
- **THEN** el error indica que falta el esquema
- **AND** NO menciona `SHUUL_JOURNALD_SOCKET`, porque la pista sería engañosa

#### Scenario: Unreadable socket
- **GIVEN** `SHUUL_JOURNALD_SOCKET` apunta a un socket existente sin permiso de conexión
- **WHEN** el lector intenta conectar
- **THEN** el warning nombra el path y el error del sistema
- **AND** el pipeline WAF sigue funcionando
