## ADDED Requirements

### Requirement: La tabla de baneos identifica cada fila de forma única

`BanManager::active_bans()` devuelve **una fila por partición `(ip_address, rule_id)`**, de
modo que una misma IP puede aparecer en dos filas y el `id` de `BanResponse` —que es la IP—
NO es único. La tabla de baneos SHALL identificar cada fila por su partición completa
`(ip_address, rule_id)` y NO por el `id` solo, porque una clave de fila repetida impide a React
reconciliar el `<tbody>` y la tabla acumula filas al navegar.

`CustomTable` SHALL permitir que la página le indique la función de clave de fila, y SHALL
conservar `record.id` como clave por defecto, de modo que las tablas cuyas filas ya tienen un
`id` único no cambien de comportamiento.

#### Scenario: Una página con dos baneos de la misma IP no duplica filas
- **GIVEN** una página de la tabla de baneos con 10 filas
- **AND** dos de esas filas son la misma `ip_address` con `rule_id` 8 y `rule_id` 6
- **WHEN** se renderiza la tabla
- **THEN** la tabla renderiza exactamente 10 filas
- **AND** React no registra el aviso de claves duplicadas

#### Scenario: La tabla no acumula filas al navegar entre páginas
- **GIVEN** una tabla de baneos cuya página contiene una IP repetida
- **WHEN** el usuario pasa a la página siguiente y vuelve a la anterior
- **THEN** el número de filas renderizadas es el tamaño de página en ambas páginas
- **AND** ese número no crece con la navegación

#### Scenario: Los baneos sin regla también se distinguen
- **GIVEN** dos baneos activos de la misma IP, uno con `rule_id` ausente y otro con `rule_id = 6`
- **WHEN** se renderiza la tabla
- **THEN** las dos filas tienen claves distintas

#### Scenario: Las tablas con id único conservan su clave (guarda de regresión)
- **GIVEN** una tabla que no declara función de clave de fila
- **WHEN** se renderiza
- **THEN** cada fila sigue usando `record.id` como clave, como hasta ahora
