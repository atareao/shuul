# Tasks: client-address-consistency

## 0. Contexto y dependencias

- [ ] 0.1 Confirmar por inspección que `NewRequest::from_request()` es el **único** punto donde se consume `X-Forwarded-For` del ForwardAuth y que tiene exactamente **tres** consumidores de la dirección: `ip_address`, `geoip.lookup` y `tor_service.is_exit_node`. Anotar aquí cualquier otro consumidor encontrado con `grep -rn 'x-forwarded-for' backend/src`.
- [ ] 0.2 Confirmar que `openspec/specs/request-extraction/` **no existe todavía** (lo materializa el archivado de `waf-input-integrity`) y que el header del requisito de este change NO coincide con ninguno del delta de `waf-input-integrity`.

## 1. RED (tests que fallan primero)

- [ ] 1.1 `models/new_request.rs`: un test que fije que, con `X-Forwarded-For: 192.0.2.1, 203.0.113.7` y un `GeoIpService` disponible, la consulta GeoIP se hace contra `203.0.113.7` y no contra la cadena. Hoy falla: `lookup` recibe la cadena.
- [ ] 1.2 Un test que fije que, con la misma cadena y un `TorService` con lista cargada, `is_tor` es `Some(_)`. Hoy falla: `is_exit_node` recibe la cadena y su `parse` devuelve `None`.
- [ ] 1.3 Un test que fije que, con `X-Forwarded-For: no-soy-una-direccion`, **no** se consulta GeoIP ni Tor: `country_code`, `country_name`, `city_name` e `is_tor` son `None`, y el log capturado contiene el diagnóstico del resolvedor (nombra `no-soy-una-direccion`) pero **no** contiene un diagnóstico de sintaxis de dirección emitido por el GeoIP.
- [ ] 1.4 Un test guardián: con `X-Forwarded-For: 8.8.8.8` (un solo valor), GeoIP y Tor se consultan contra `8.8.8.8` — hoy pasa.
- [ ] 1.5 Ejecutar `cargo test` desde `backend/` y confirmar que 1.1–1.3 fallan y 1.4 pasa.

## 2. GREEN (implementación mínima)

- [ ] 2.1 `new_request.rs`: resolver la dirección una sola vez y pasar `ip_address.as_deref()` a los tres consumidores. GeoIP y Tor solo se consultan cuando `ip_address` es `Some`.
- [ ] 2.2 Ejecutar `cargo test` desde `backend/` y confirmar 100 % verde.

## 3. REFACTOR

- [ ] 3.1 `cargo fmt --check` limpio y `cargo clippy --lib --bins -- -D warnings` limpio.
- [ ] 3.2 Comparar el conteo de avisos de `cargo clippy --all-targets` contra la base preexistente y confirmar 0 añadidos.
- [ ] 3.3 Confirmar que `backend/src/journald_reader.rs` sigue intacto.

## 4. Documentación

- [ ] 4.1 Anotar en `WAF-ROADMAP.md` (sección de diagnóstico) el hallazgo medido: el GeoIP y el Tor consumían la cabecera cruda y quedaban en fail-open con una cadena.
- [ ] 4.2 Confirmar que la guía y el roadmap siguen declarando el orden correcto: resolución de la dirección → consumidores → confianza en la cabecera.

## 5. Verificación manual (operador, en `co1`)

- [ ] 5.1 Desplegar la imagen nueva y repetir la inyección de una cadena (`X-Forwarded-For: 192.0.2.1, 203.0.113.7`): el registro SHALL mostrar `ip` de un solo valor y NO SHALL aparecer `ERROR … invalid IP address syntax`.
- [ ] 5.2 Confirmar que las reglas por país vuelven a casar para una petición con cadena.
- [ ] 5.3 **Solo entonces**, retomar la tarea 5.3 de `waf-input-integrity`: activar `trustForwardHeader: true` con `forwardedHeaders.trustedIPs` y verificar que la dirección que Shuul registra sigue siendo la del cliente.
