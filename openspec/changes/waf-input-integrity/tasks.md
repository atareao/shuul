# Tasks: waf-input-integrity

## 0. Medición previa (resuelve una decisión abierta)

- [ ] 0.1 Medir si Traefik **añade** su elemento al final de `X-Forwarded-For` o lo **antepone**, con un Traefik de pruebas —**nunca en producción**— configurado con `trustForwardHeader: true` y una petición que ya lleve la cabecera. Anotar el valor observado en esta tarea.
- [x] 0.2 La elección del **último** elemento la ha fijado el operador (2026-10-24) y NO SHALL cambiar por la medición por sí sola: si 0.1 la contradice, se anota y se vuelve al operador antes de tocar el test o la spec.

## 1. RED (tests que fallan primero)

- [x] 1.1 `models/new_request.rs`: un test que fije que `X-Forwarded-For: 192.0.2.1, 203.0.113.7` produce `ip_address = Some("203.0.113.7")` — el último elemento, nunca la cadena entera.
- [x] 1.2 Un test con espacios alrededor de los elementos (`192.0.2.1 ,  203.0.113.7`) que fije el recorte.
- [x] 1.3 Un test con una dirección IPv6 (`2001:db8::1`): el recorte por comas no debe romper los dos puntos.
- [x] 1.4 Un test con un valor que no es una dirección: `ip_address = None` **y** diagnóstico emitido.
- [x] 1.5 Un test con una cadena cuya cola no es una dirección: `ip_address = None` y diagnóstico emitido.
- [x] 1.6 Los tests de la cabecera ausente y de la cabecera vacía siguen en verde y **no** emiten diagnóstico.
- [x] 1.7 `cargo test --no-fail-fast` desde `backend/`: los tests nuevos fallan por la aserción —no por compilación— y los preexistentes siguen en verde (384 hoy).

## 2. GREEN (implementación mínima)

- [x] 2.1 `models/new_request.rs`: interpretar `X-Forwarded-For` como cadena —tomar el último elemento, recortarlo y validarlo como `IpAddr`— en lugar de copiar el valor crudo.
- [x] 2.2 Emitir el diagnóstico cuando el elemento elegido no sea una dirección válida, nombrando el valor descartado. Sin diagnóstico para el caso ausente ni para el vacío.
- [x] 2.3 `cargo test --no-fail-fast` en verde y `cargo check` limpio.

## 3. REFACTOR

- [x] 3.1 `cargo clippy --all-targets --all-features -- -D warnings` y `cargo fmt -- --check` limpios.
- [x] 3.2 Comprobar que `journald_reader.rs` **no** se ha tocado: su parser de `ClientHost`/`ClientAddr` es la referencia y ya es correcto.
- [x] 3.3 Comprobar que ninguna regla existente cambia de comportamiento con direcciones válidas: el contrato se estrecha, no se mueve.

## 4. Documentación

- [x] 4.1 `SELF-HOSTING.md` §1.2: sustituir `trustForwarders: true` por la receta correcta —`trustForwardHeader: true` junto con `forwardedHeaders.trustedIPs` en el entrypoint—.
- [x] 4.2 `SELF-HOSTING.md` §8.1: el middleware de esa sección no lleva la opción; añadir la misma receta.
- [x] 4.3 `SELF-HOSTING.md` §1: nuevos apartados con los ajustes de endurecimiento del entrypoint y sus valores —`aliasHeadersStrategy`, `encodedCharacters`, `maxHeaderBytes`— y **el orden**: primero la resolución de la dirección a un solo valor, después la confianza en la cabecera.
- [x] 4.4 `SELF-HOSTING.md` §2.15 y `WAF-ROADMAP.md`: actualizar la referencia a `traefik:v3.7.5` por la versión que corre en producción.
- [x] 4.5 Declarar en la guía que la dirección solo es fiable si el despliegue elimina las `X-Forwarded-*` que envía el cliente.
- [ ] 4.6 Redactar el `Purpose` de `request-extraction` **después** de archivar: el archivado de una capability nueva lo deja como marcador y un delta no puede reemplazarlo.

## 5. Verificación manual (operador, en `co1`)

- [x] 5.1 Desplegar la imagen nueva y confirmar que los pipelines WAF y Jail siguen vivos y que el Jail sigue leyendo el access log. **Medido el 2026-10-24 con v0.23.0 (digest `sha256:754504ae…`)**: contenedor `Up (healthy)`, OIDC inicializado, `journald reader: streaming access logs from docker:unix:/var/run/docker.sock (traefik)`, WAF emitiendo `[ADMITTED]`/`[UNMATCHED]`/`[DENIED]` y Jail emitiendo `[REGISTERED]`/`[CLEARED]`.
- [x] 5.2 Comprobar en los logs que una petición cuya `X-Forwarded-For` llegue en forma de cadena registra **una sola dirección** —la del último elemento— y no la cadena entera. **Medido el 2026-10-24**: inyectando `X-Forwarded-For: 192.0.2.1, 203.0.113.7` al handler, el registro de auditoría muestra `"ip":"203.0.113.7"` — el último elemento, una sola dirección.
- [ ] 5.3 Solo con el código nuevo desplegado: aplicar `trustForwardHeader: true` con `forwardedHeaders.trustedIPs` en Traefik y **verificar que la dirección que Shuul registra sigue siendo la del cliente**. Ese es el momento de callar el aviso, y no antes. **BLOQUEADA (2026-10-24)** por el hallazgo del final verification: el GeoIP y el filtro de Tor siguen consumiendo la cabecera cruda (`new_request.rs`: `geoip.lookup(ip)` e `is_exit_node(ip)`), así que activar `trustForwardHeader` —que introduce cadenas— los rompería en fail-open. Se retoma cuando aterrice el change `client-address-consistency`, que cierra ese hueco.