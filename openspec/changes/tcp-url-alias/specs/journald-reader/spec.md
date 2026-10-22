## ADDED Requirements

### Requirement: TCP transport URL scheme

Los transportes TCP se configuran con una **URL base** en `SHUUL_DOCKER_URL` (API de
Docker) y en `SHUUL_JOURNALD_URL` (gatewayd). Ambas SHALL aceptar `http://` y `tcp://`
como **sinónimos**, de modo que la sintaxis que Docker emplea para `DOCKER_HOST`
(`tcp://host:puerto`) SHALL valer sin traducción y SHALL producir exactamente el mismo
transporte, la misma autoridad y la misma petición que su equivalente en `http://`.

`https://` SHALL rechazarse con un diagnóstico que lo nombre, porque los transportes TCP
NO implementan TLS. Cualquier otro esquema SHALL rechazarse con un diagnóstico que
nombre el esquema recibido, y una URL sin esquema SHALL rechazarse nombrando la
variable que la contiene. En los tres casos el rechazo SHALL ser **recuperable**: un
warning y un reintento tras `retry_delay`, nunca la caída del proceso ni un error que
impida el arranque.

Cuando la URL traiga un **path base**, éste SHALL ignorarse al construir la petición y
SHALL registrarse un warning nombrándolo, como ya ocurre con los demás endpoints TCP.

`endpoint_display()` SHALL mostrar el **esquema efectivo**, no el escrito: una URL
configurada como `tcp://host:2375` SHALL mostrarse como `http://host:2375` —y como
`docker:http://host:2375 ({container_name})` en el backend de Docker—, de modo que los
logs no exhiban un esquema que la conexión no usa.

#### Scenario: tcp:// is accepted as an alias of http://
- **GIVEN** `SHUUL_DOCKER_URL=tcp://docker-socket-proxy:2375` definida y no vacía
- **AND** `SHUUL_DOCKER_SOCKET` sin definir
- **WHEN** se construye el reader con `from_env()`
- **THEN** el transporte es `DockerTcp` con autoridad `docker-socket-proxy:2375`
- **AND** la petición se envía sobre HTTP/1.1 en claro, idéntica a la de `http://`

#### Scenario: The alias also works for the gatewayd URL
- **GIVEN** `SHUUL_JOURNALD_URL=tcp://journald.example:19531` definida y no vacía
- **AND** ninguna variable de mayor precedencia definida
- **WHEN** se construye el reader con `from_env()`
- **THEN** el transporte es `Tcp` con autoridad `journald.example:19531`

#### Scenario: The effective scheme is what gets displayed
- **GIVEN** una URL base configurada como `tcp://docker-socket-proxy:2375`
- **WHEN** se llama a `endpoint_display()`
- **THEN** devuelve `docker:http://docker-socket-proxy:2375 (traefik)`
- **AND** el esquema `tcp` no aparece en la línea de arranque

#### Scenario: https is still rejected by name
- **GIVEN** `SHUUL_JOURNALD_URL=https://journald.example:19531` definida y no vacía
- **WHEN** se construye el reader con `from_env()`
- **THEN** el intento de conexión falla con un diagnóstico que nombra `https`
- **AND** el diagnóstico no se confunde con un esquema desconocido
- **AND** el reader reintenta tras `retry_delay` sin tumbar el proceso

#### Scenario: An unknown scheme is rejected by name
- **GIVEN** `SHUUL_DOCKER_URL=ftp://host:2375` definida y no vacía
- **WHEN** se construye el reader con `from_env()`
- **THEN** el rechazo nombra el esquema `ftp`
- **AND** el reader reintenta tras `retry_delay` sin tumbar el proceso

#### Scenario: A URL without a scheme is rejected naming the variable
- **GIVEN** `SHUUL_DOCKER_URL=host:2375` definida y no vacía
- **WHEN** se construye el reader con `from_env()`
- **THEN** el rechazo nombra la variable que contiene la URL
- **AND** el reader reintenta tras `retry_delay` sin tumbar el proceso

#### Scenario: A base path is ignored with a warning
- **GIVEN** `SHUUL_DOCKER_URL=tcp://docker-socket-proxy:2375/docker` definida y no vacía
- **WHEN** se construye el reader
- **THEN** la URL efectiva es `http://docker-socket-proxy:2375`
- **AND** se registra un warning nombrando el path base ignorado
