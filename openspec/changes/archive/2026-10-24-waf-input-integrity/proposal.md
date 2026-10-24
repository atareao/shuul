# Proposal: waf-input-integrity

## Why

**La dirección del cliente con la que el WAF decide no está garantizada.** `NewRequest::from_request()` copia el valor crudo de `X-Forwarded-For` a `ip_address`, sin partirlo y sin validarlo. Si esa cabecera trae una cadena —`1.2.3.4, 5.6.7.8`— el campo guarda la cadena entera, y aguas abajo se cae a la vez todo lo que depende de la dirección:

- la **comprobación de baneos** hace `parse::<IpAddr>()`, falla y **se salta entera** — la petición pasa;
- el **GeoIP** no resuelve, así que las reglas por país y por ciudad dejan de casar;
- el **filtro de Tor** no consulta;
- las **reglas por dirección** dejan de casar.

Medido en producción el 2026-10-24: `GET /api/v1/util/complete?ip=85.214.132.117,%20192.0.2.1` devuelve `country_code: null` y el log registra `ERROR … invalid IP address syntax`. El campo guarda literalmente `85.214.132.117, 192.0.2.1`.

**Hoy no es explotable desde fuera, y esto también está medido.** Una petición con `X-Forwarded-For: 85.214.132.117` —una dirección de un país bloqueado— devolvió 200 y Shuul registró `ip: 192.168.224.1`: Traefik elimina la cabecera que envía el cliente y pone la suya. **Pero esa protección es del despliegue, no del código**, y el propio Traefik recomienda retirarla: el aviso de arranque de `trustForwardHeader` propone ponerlo a `true`, que es justo lo que deja de eliminar la cabecera del cliente.

**Y la guía de despliegue no ayuda, más bien al contrario.** `SELF-HOSTING.md` documenta `trustForwarders: true`, una opción que **no existe**; la correcta es `trustForwardHeader`. Traefik la ignora en silencio, así que el operador se queda con el comportamiento que el aviso describe como *«inconsistente»* y el aviso no se calla nunca. Lo que se pone en marcha siguiendo la guía no es lo que la guía dice. El operador ya lo tiene anotado sin resolver.

**Por qué ahora.** La actualización a Traefik 3.7.16 del 2026-10-24 ha dejado los tres avisos a la vista: `aliasHeadersStrategy` (7 entrypoints), `trustForwardHeader` (todos los routers) y `maxResponseBodySize`. Arreglarlo en el orden equivocado —poniendo `trustForwardHeader: true` antes de arreglar el parseo— convertiría un fallo latente en un fail-open del sistema de baneos.

## What Changes

1. **La dirección del cliente se resuelve a una sola dirección.** `NewRequest::from_request()` interpreta `X-Forwarded-For` como una **cadena** de saltos, no como un valor opaco: toma el elemento que observó el salto más próximo —el último—, lo recorta y lo valida como `IpAddr`. El campo `ip_address` nunca vuelve a contener una lista, una coma ni un espacio.
2. **Una cadena que no se puede resolver no se cuela.** Si el elemento elegido no es una dirección válida, `ip_address` queda en `None` y se emite un diagnóstico que nombra el valor descartado —el mismo patrón que la spec `rule-matching` fijó para los campos restringidos ausentes—.
3. **La guía de despliegue dice la verdad.** Se corrige `trustForwarders` por `trustForwardHeader`, se declara que la dirección que Shuul comprueba solo es fiable si el despliegue elimina las cabeceras `X-Forwarded-*` que envía el cliente, y se declara el orden: primero la resolución de la dirección a un solo valor, después la confianza en la cabecera.
4. **Se documentan los ajustes de endurecimiento del entrypoint** que la actualización ha dejado a la vista, con sus valores, y se marcan las referencias a la versión de Traefik que han quedado obsoletas.

## Capabilities

**New Capabilities**

- `request-extraction` — cómo se construye la vista de la petición que el WAF evalúa a partir de las cabeceras de reenvío. Hoy **no tiene spec**: `NewRequest::from_request()` es la función de la que dependen todas las reglas y su contrato no está escrito en ninguna parte. Esta capability le da casa, y es la que absorberá las colecciones nuevas del roadmap —cookies, cabeceras con nombre, argumentos sueltos—.

**Modified Capabilities**

- Ninguna. El matcheo (`rule-matching`), el orden de evaluación (`waf-pipeline`) y la integración de Tor (`tor-filter`) no cambian: cambia de dónde sale el valor del campo, no qué se hace con él.

## Impact

- **Backend**: `models/new_request.rs`, `from_request()` — el único fichero de código que cambia. Un `split`, un `trim` y un `parse`.
- **Docs**: `SELF-HOSTING.md` (§1.2 y §8.1, la opción mal nombrada; §1, los ajustes de endurecimiento; §2.15, la referencia a la versión de Traefik) y `WAF-ROADMAP.md` (la referencia a la versión).
- **Spec**: se crea `request-extraction`.
- **Datos y API**: sin migración, sin cambio de API, sin cambio de esquema. El contrato del campo `ip_address` se **estrecha** —una dirección, no cualquier cadena—, lo que es compatible hacia atrás con todo lo que hoy funciona: las direcciones válidas siguen viajando igual.
- **Fuera de alcance**: el endurecimiento de Traefik en sí (vive en `co1`, fuera del repo), la revisión de la plantilla «Geo blocking» (otra decisión) y las transformaciones (`F1` del roadmap).

## Decisions taken when opening the change

- **Se toma el ÚLTIMO elemento de la cadena, no el primero.** Es el que observó el salto más próximo y, por tanto, **el mismo del que el pipeline Jail deriva la dirección** al leer `ClientHost`/`ClientAddr` del access log. Esa coincidencia no es un detalle: si el WAF comprueba los baneos contra una dirección distinta de la que el Jail usó para crearlos, los baneos no se aplican nunca. Tomar el primero —la lectura convencional— daría la dirección que un cliente puede anteponer, con dos consecuencias malas: el atacante evade su propio baneo y consigue que se banee a un tercero.
- **La elección de extremo queda fijada por un test y pendiente de medida.** No está verificado si Traefik **añade** su elemento al final —lo que hace correcto tomar el último— o lo **antepone**; hoy no se puede observar porque elimina la cabecera del cliente y deja un solo elemento. La tarea 0.1 mide esto en un Traefik de pruebas, nunca en producción. Si la medición contradice la elección, se corrige el test y la spec con la medición delante.
- **Un valor irresoluble queda en `None` con diagnóstico, no se adivina.** No hay de dónde sacar la dirección si la cabecera no la trae: un ForwardAuth no ve el par TCP. Inventar un valor —tomar el primer elemento válido de la cadena, por ejemplo— reintroduciría justo lo que se quiere cerrar. Se deja constancia con un diagnóstico y se acepta que esa petición no se pueda comprobar contra los baneos.
- **El endurecimiento de Traefik no se hace en este change, y el orden se escribe.** Cambiar `trustForwardHeader` es un cambio en el despliegue del operador (`co1`), fuera del repositorio. Lo que este change hace es dejar el código preparado y escribir el orden correcto, para que nadie lo active al revés.
- **`trustForwardHeader` se nombra bien, no se silencia.** Callar el aviso sin más no arregla nada. La receta que se documenta es la que Traefik recomienda: `trustForwardHeader: true` **junto con** `forwardedHeaders.trustedIPs` en el entrypoint, para que sea el entrypoint quien elimine las cabeceras de los clientes no fiables.

## Límites que se aceptan

- **Con la dirección en `None`, la petición no se comprueba contra los baneos.** Es un fail-open y se documenta como tal. No se puede cerrar sin ver el par TCP, que un ForwardAuth no da.
- **No se añade un `X-Real-Ip` ni un mecanismo alternativo.** El despliegue manda `X-Forwarded-For`; inventar una segunda fuente sería otro contrato que mantener.
- **No se toca `journald_reader.rs`.** Su parser de `ClientHost`/`ClientAddr` ya es correcto —prefiere `ClientHost`, y el fallback quita puerto y corchetes IPv6— y es justo la referencia contra la que el WAF tiene que coincidir.
