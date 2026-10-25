## ADDED Requirements

### Requirement: La extracción resuelve las colecciones nuevas del WAF

La extracción de la petición del pipeline del WAF SHALL resolver, además de los campos actuales, las colecciones siguientes a partir del encabezado HTTP recibido:

- **cookies:** los pares `nombre=valor` del encabezado `Cookie`, con el nombre como clave.
- **cabeceras:** todas las cabeceras de la petición, con el nombre en minúsculas como clave y su valor.
- **argumentos del query:** los pares `nombre=valor` del *query string*, con el nombre como clave y el valor **crudo** (sin decodificar), igual que el campo `query`.
- **basename:** el último segmento no vacío del path.
- **extensión:** la subcadena tras el último punto del basename, cuando ese punto no es el primer carácter.

Los valores de los argumentos del query SHALL llegar **crudos** (codificados), igual que el campo `query`, para que la pipeline de transformaciones los decodifique.

Una entrada ausente o con valor vacío SHALL representarse como ausencia: no SHALL aparecer en la colección.

#### Scenario: Las cookies se extraen del encabezado Cookie

- **GIVEN** una petición con `Cookie: a=1; b=2`
- **THEN** la colección de cookies SHALL contener `a→1` y `b→2`

#### Scenario: Las cabeceras se extraen con el nombre en minúsculas

- **GIVEN** una petición con `Authorization: Bearer x`
- **THEN** la colección de cabeceras SHALL contener `authorization→Bearer x`

#### Scenario: Los argumentos del query se extraen uno a uno y crudos

- **GIVEN** una petición con `?id=1%20union&x=2`
- **THEN** la colección de argumentos SHALL contener `id→1%20union` y `x→2`
- **AND** el valor de `id` SHALL conservar el `%20` sin decodificar

#### Scenario: El basename y la extensión se derivan del path

- **GIVEN** una petición con path `/a/b/index.php`
- **THEN** el basename SHALL ser `index.php`
- **AND** la extensión SHALL ser `php`

#### Scenario: No hay basename ni extensión sin último segmento

- **GIVEN** una petición cuyo path es `/`
- **THEN** el basename y la extensión SHALL estar ausentes

#### Scenario: La extracción del Jail conserva lo derivable de su payload

- **GIVEN** una petición del pipeline Jail cuyo payload trae `path` y `query`
- **THEN** los argumentos del query, el basename y la extensión SHALL derivarse de él
- **AND** las cookies y las cabeceras SHALL quedar ausentes, porque su payload no las trae
