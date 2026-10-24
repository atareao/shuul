# Tasks: client-address-consistency

## 0. Contexto y dependencias

- [x] 0.1 Confirmar por inspección que `NewRequest::from_request()` es el **único** punto donde se consume `X-Forwarded-For` del ForwardAuth y que tiene exactamente **tres** consumidores de la dirección: `ip_address`, `geoip.lookup` y `tor_service.is_exit_node`. Anotar aquí cualquier otro consumidor encontrado con `grep -rn 'x-forwarded-for' backend/src`. **Verificado (2026-10-24)**: `grep -rin forwarded backend/src` solo devuelve `models/new_request.rs` (producción y tests) y los tests de `http/shuul.rs`; el único `NewRequest` construido fuera es `http/report.rs`, y parte de `payload.ip_address` —ya resuelto por el `JournaldReader`—, no de la cabecera.
- [x] 0.2 Confirmar que `openspec/specs/request-extraction/` **no existe todavía** (lo materializa el archivado de `waf-input-integrity`) y que el header del requisito de este change NO coincide con ninguno del delta de `waf-input-integrity`. **Verificado (2026-10-24)**: `openspec/specs/` no contiene `request-extraction`; el header de este change —«Todo consumidor de la dirección del cliente usa la dirección resuelta»— es distinto de «La dirección del cliente se resuelve a una sola dirección» y de «La cabecera no se cree por defecto: se cree si el despliegue lo declara».

## 1. RED (tests que fallan primero)

- [x] 1.1 `models/new_request.rs`: un test que fije que, con `X-Forwarded-For: 192.0.2.1, 8.8.8.8` y un `GeoIpService` disponible, la consulta GeoIP se hace contra `8.8.8.8` y no contra la cadena. **Medido (2026-10-24)**: falla en `new_request.rs:548` — `country_code` es `None` porque el GeoIP recibe la cadena; log capturado: `ERROR backend::models::ipdata: Look data for ip: "192.0.2.1, 8.8.8.8": invalid IP address syntax`.
- [x] 1.2 Un test que fije que, con la misma cadena y un `TorService` con lista cargada, `is_tor` es `Some(_)`. **Medido**: falla en `new_request.rs:571` — `assert_eq!(request.is_tor, Some(true))` da `left: None`; el `parse::<IpAddr>()` de la cadena falla en `is_exit_node`.
- [x] 1.3 Un test que fije que, con `X-Forwarded-For: no-soy-una-direccion`, **no** se consulta GeoIP ni Tor: `country_code`, `country_name`, `city_name` e `is_tor` son `None`, y el log capturado contiene el diagnóstico del resolvedor pero **no** un diagnóstico de sintaxis de dirección emitido por el GeoIP. **Medido**: falla en `new_request.rs:614` — el log traía el `DEBUG` del resolvedor **y** el `ERROR` del GeoIP.
- [x] 1.4 Un test guardián: con `X-Forwarded-For: 8.8.8.8` (un solo valor), GeoIP y Tor se consultan contra `8.8.8.8`. **Medido**: pasa.
- [x] 1.5 Ejecutar `cargo test` desde `backend/` y confirmar que 1.1–1.3 fallan y 1.4 pasa. **Medido**: `--lib` 15 verdes / 3 rojos; los tres rojos son los tests nuevos, y fallan por aserción, no por compilación.

## 2. GREEN (implementación mínima)

- [x] 2.1 `new_request.rs`: resolver la dirección una sola vez y pasar `ip_address.as_deref()` a los tres consumidores. GeoIP y Tor solo se consultan cuando `ip_address` es `Some`.
- [x] 2.2 Ejecutar `cargo test` desde `backend/` y confirmar 100 % verde. **Medido**: 414 verdes / 0 rojos.

## 3. REFACTOR

- [x] 3.1 `cargo fmt --check` limpio y `cargo clippy --lib --bins -- -D warnings` limpio.
- [x] 3.2 Comparar el conteo de avisos de `cargo clippy --all-targets` contra la base preexistente y confirmar 0 añadidos. **Medido**: 83 = base (se detectaron 5 avisos `doc_markdown` nuevos por `GeoIP` sin backticks en los doc-comments de test, y se corrigieron).
- [x] 3.3 Confirmar que `backend/src/journald_reader.rs` sigue intacto.
- [x] 3.4 Atender los hallazgos de la auditoría de `rust-reviewer`: M1 (añadir las aserciones de `country_name` y `city_name` en el escenario irresoluble), M2 (fallo explícito con mensaje accionable si falta el mmdb, en vez del salto silencioso que dejaba el test vacío), O1 (renombrar el binding del `match` a `resolved_ip` para no sombrear `ip_address`), O2 (precisar el doc: solo hay diagnóstico si había un valor no vacío que no resolvió) y O4 (retirar `clippy::option_if_let_else` del `#[allow]`, ya sin uso tras sustituir los `if let … else` por `match`).

## 4. Documentación

- [x] 4.1 Anotar en `WAF-ROADMAP.md` (sección de diagnóstico) el hallazgo medido: el GeoIP y el filtro de Tor consumían la cabecera cruda y quedaban en fail-open con una cadena. **Hecho**: nuevo apartado §1.4.
- [x] 4.2 Confirmar que la guía y el roadmap siguen declarando el orden correcto: resolución de la dirección → consumidores → confianza en la cabecera.

## 5. Verificación manual (operador, en `co1`)

- [ ] 5.1 Desplegar la imagen nueva y repetir la inyección de una cadena (`X-Forwarded-For: 192.0.2.1, 203.0.113.7`): el registro SHALL mostrar `ip` de un solo valor y NO SHALL aparecer `ERROR … invalid IP address syntax`.
- [ ] 5.2 Confirmar que las reglas por país vuelven a casar para una petición con cadena.
- [ ] 5.3 **Solo entonces**, retomar la tarea 5.3 de `waf-input-integrity`: activar `trustForwardHeader: true` con `forwardedHeaders.trustedIPs` y verificar que la dirección que Shuul registra sigue siendo la del cliente.
