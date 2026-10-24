# request-extraction Specification (delta)

## ADDED Requirements

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
