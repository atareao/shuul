# request-extraction Specification

## Purpose
Define cómo se construye la vista de la petición que el WAF y el Jail evalúan a partir de las cabeceras de reenvío: qué campos se extraen, de dónde sale cada uno, y —de forma señalada— cómo se resuelve la dirección del cliente a una sola dirección y de quién se fía esa cabecera.

## Requirements

### Requirement: La dirección del cliente se resuelve a una sola dirección

`NewRequest::ip_address` SHALL contener **exactamente una** dirección IP, o `None`. NO SHALL contener nunca una lista, una coma ni espacios.

`X-Forwarded-For` SHALL interpretarse como una **cadena** de saltos, no como un valor opaco. Cuando traiga varios elementos separados por comas, la dirección SHALL ser **el último elemento**, por ser el que observó el salto más próximo: el mismo del que el pipeline Jail deriva la dirección al leer el access log. Los elementos que un cliente pueda anteponer NO SHALL decidir la dirección.

Los espacios alrededor del elemento elegido SHALL ignorarse.

Cuando la cabecera falte, esté vacía, o el elemento elegido NO sea una `IpAddr` válida, `ip_address` SHALL ser `None` y SHALL emitirse un diagnóstico que nombre el valor descartado. Una cabecera ausente o vacía NO SHALL producir diagnóstico.

#### Scenario: Un solo valor
- **WHEN** la petición trae `X-Forwarded-For: 203.0.113.7`
- **THEN** `ip_address` SHALL ser `Some("203.0.113.7")`

#### Scenario: Una cadena de saltos
- **WHEN** la petición trae `X-Forwarded-For: 192.0.2.1, 203.0.113.7`
- **THEN** `ip_address` SHALL ser `Some("203.0.113.7")`
- **AND** NO SHALL ser `"192.0.2.1, 203.0.113.7"`

#### Scenario: Una cadena con espacios alrededor de los elementos
- **WHEN** la petición trae `X-Forwarded-For: 192.0.2.1 ,  203.0.113.7`
- **THEN** `ip_address` SHALL ser `Some("203.0.113.7")`

#### Scenario: Una dirección IPv6
- **WHEN** la petición trae `X-Forwarded-For: 2001:db8::1`
- **THEN** `ip_address` SHALL ser `Some("2001:db8::1")`

#### Scenario: Un valor que no es una dirección
- **WHEN** la petición trae `X-Forwarded-For: no-soy-una-direccion`
- **THEN** `ip_address` SHALL ser `None`
- **AND** SHALL emitirse un diagnóstico que nombre el valor descartado

#### Scenario: Una cadena cuya cola no es una dirección
- **WHEN** la petición trae `X-Forwarded-For: 192.0.2.1, no-soy-una-direccion`
- **THEN** `ip_address` SHALL ser `None`
- **AND** SHALL emitirse un diagnóstico que nombre el valor descartado

#### Scenario: Cabecera ausente
- **WHEN** la petición no trae `X-Forwarded-For`
- **THEN** `ip_address` SHALL ser `None`
- **AND** NO SHALL emitirse diagnóstico

#### Scenario: Cabecera vacía
- **WHEN** la petición trae `X-Forwarded-For` con el valor vacío
- **THEN** `ip_address` SHALL ser `None`
- **AND** NO SHALL emitirse diagnóstico

#### Scenario: La dirección coincide con la que usa el pipeline Jail
- **WHEN** el pipeline Jail registra un baneo para la dirección que deriva del access log de una petición
- **AND** esa misma petición llega después al pipeline WAF
- **THEN** la comprobación de baneos SHALL evaluarse contra esa misma dirección
- **AND** NO SHALL quedar sin evaluar por un valor que no se pueda parsear

### Requirement: La cabecera no se cree por defecto: se cree si el despliegue lo declara

La fiabilidad de `X-Forwarded-For` NO SHALL depender de que el cliente se abstenga de enviarla, sino de que el despliegue la elimine. Shuul NO SHALL asumir que la cabecera que recibe procede de un salto de confianza.

La guía de despliegue SHALL nombrar la opción de Traefik que gobierna esa confianza con su nombre correcto —`trustForwardHeader`— y NO SHALL nombrar una opción inexistente.

La guía SHALL declarar el orden: primero la resolución de la dirección a un solo valor, después la confianza en la cabecera del despliegue.

#### Scenario: La guía nombra la opción correcta
- **WHEN** se inspecciona la guía de despliegue
- **THEN** SHALL nombrar `trustForwardHeader`
- **AND** NO SHALL nombrar `trustForwarders` ni ninguna otra opción que Traefik no reconozca

#### Scenario: La guía declara de quién se fía
- **WHEN** se inspecciona la guía de despliegue
- **THEN** SHALL declarar que la dirección que Shuul comprueba solo es fiable cuando el despliegue elimina las cabeceras `X-Forwarded-*` que envía el cliente

#### Scenario: La guía declara el orden
- **WHEN** se inspecciona la guía de despliegue
- **THEN** SHALL declarar que la resolución de la dirección a un solo valor va antes que la confianza en la cabecera

### Requirement: Todo consumidor de la dirección del cliente usa la dirección resuelta

`NewRequest::from_request()` SHALL resolver la dirección del cliente **una sola vez** y SHALL usar ese mismo valor en **todos** los consumidores que dependan de ella: el campo `ip_address`, la consulta al servicio GeoIP y la consulta al filtro de Tor. NO SHALL entregar la cabecera `X-Forwarded-For` sin resolver a ninguno de ellos.

El valor que SHALL llegar a esos consumidores es el de `ip_address`: **exactamente una** dirección, ya recortada y validada como `IpAddr`, según el requisito «La dirección del cliente se resuelve a una sola dirección».

Cuando `ip_address` sea `None` —cabecera ausente, vacía o irresoluble— la consulta GeoIP y la consulta de Tor NO SHALL ejecutarse, y `city_name`, `country_name`, `country_code` e `is_tor` SHALL ser `None`. NO SHALL emitirse un diagnóstico de sintaxis de dirección procedente de los consumidores.

#### Scenario: Una cadena de saltos con GeoIP disponible
- **WHEN** la petición trae `X-Forwarded-For: 192.0.2.1, 203.0.113.7` y hay un servicio GeoIP disponible
- **THEN** la consulta GeoIP SHALL hacerse contra `203.0.113.7`
- **AND** NO SHALL hacerse contra `"192.0.2.1, 203.0.113.7"`

#### Scenario: Una cadena de saltos con el filtro de Tor disponible
- **WHEN** la petición trae `X-Forwarded-For: 192.0.2.1, 203.0.113.7` y hay un servicio Tor con su lista cargada
- **THEN** la consulta de Tor SHALL hacerse contra `203.0.113.7`
- **AND** `is_tor` SHALL ser `Some(_)`
- **AND** NO SHALL ser `None` por culpa de la cadena

#### Scenario: Una dirección irresoluble no se consulta
- **WHEN** la petición trae `X-Forwarded-For: no-soy-una-direccion`
- **THEN** la consulta GeoIP NO SHALL ejecutarse
- **AND** la consulta de Tor NO SHALL ejecutarse
- **AND** `country_code`, `country_name`, `city_name` e `is_tor` SHALL ser `None`

#### Scenario: Un solo valor sigue resolviéndose igual
- **WHEN** la petición trae `X-Forwarded-For: 8.8.8.8`
- **THEN** la consulta GeoIP y la consulta de Tor SHALL hacerse contra `8.8.8.8`
