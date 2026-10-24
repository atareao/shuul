# Proposal: ban-row-identity

## Why

**El síntoma, tal cual lo reportó el usuario sobre el despliegue de la `v0.17.0` en producción:**

> «La primera vez salen 10 en la primera página, cuando paso a la siguiente página hay 12,
> si paso a la siguiente y vuelvo hacia atrás hay 14… y así sucesivamente»

El número de filas **crece de forma acumulativa** en cada navegación, y el usuario confirma
que **en el listado ve la misma IP dos veces**.

**La paginación no tiene la culpa.** `GET /api/v1/bans` pagina correctamente: `list_handler`
(`backend/src/http/ban.rs:189-218`) aplica `skip`/`take`, devuelve el `limit` pedido y un
`PagedResponse` con `{page, limit, pages, records, prev, next}`, que es exactamente lo que
`CustomTable` lee. El change `admin-ui-fixes` —ya publicado como `v0.17.0`— reescribió el
paginador de `CustomTable` por mecanismos propios **verificados**, y dejó escrito que el
síntoma literal **no se reproducía** (`openspec/changes/admin-ui-fixes/proposal.md:26-34`).
No lo reprodujo porque lo midió con 34 registros **sin identidades repetidas**: la colisión
nunca entró en la medición. El defecto nunca estuvo en el paginador.

**Causa raíz: dos filas distintas comparten identificador.**

1. `BanManager::active_bans()` (`backend/src/models/ban_manager.rs:222-232`) devuelve **una
   entrada por partición `(ip, rule_id)`**, no por IP: el contrato de persistencia mantiene un
   baneo vivo por partición y la base lo respalda con el índice
   `UNIQUE (ip_address, rule_id) WHERE expired = 0`. La propia spec lo dice: *«Una IP puede
   estar baneada por varias rules distintas»* (`openspec/specs/ban-manager/spec.md:67`).
2. `list_handler` (`backend/src/http/ban.rs:85-87`) construye `id: ip.to_string()`. Es decir,
   **dos baneos de la misma IP salen con el mismo `id`**.
3. `CustomTable<T extends { id: number | string }>` los usa como `rowKey`
   (`frontend/src/components/custom_table.tsx:575`): el contrato del componente exige que el
   `id` sea único. Con dos hijos con la misma `key`, React no puede reconciliar el `<tbody>` y
   el DOM **acumula** filas en cada render: 10, 12, 14…

**Evidencia medida en producción** (tabla `bans` del volumen `shuul_db`, en solo lectura):

| Medición | Valor |
|---|---|
| filas con `expired = 0` | 33 |
| IPs distintas | **29** |
| IPs con dos baneos activos | **4** |

Las cuatro: `130.12.180.117` (rules 8 y 6), `195.178.110.108` (6 y 9), `35.205.131.43`
(8 y 6) y `37.19.64.160` (8 y 6).

**La identidad de un baneo ya está definida en la spec**: es la partición `(id, rule_id)` —el
propio endpoint DELETE exige `rule_id` para identificar cuál eliminar—. Es el frontend el que
la incumple al usar sólo `id` como clave de fila.

## What Changes

- **La tabla de baneos pasa a identificar cada fila por su partición completa
  `(ip_address, rule_id)`**, no por el `id` solo.
- `CustomTable` expone una prop **opcional** `rowKey` —`(record: T) => string | number`— cuyo
  valor por defecto sigue siendo `record.id`, de modo que **ninguna otra tabla cambia de
  comportamiento**. `BansPage` pasa su partición.

Sin cambios en el backend, en la API, en la base de datos ni en las migraciones: `BanResponse`
ya trae `ip_address` y `rule_id`, que son los dos campos que identifican la partición.

## Non-goals

- **No se cambia `BanResponse.id`.** Se evaluó y se descarta: la spec documenta el `id` como la
  IP (`DELETE /api/v1/bans?id=1.2.3.4&rule_id=3`) y la identidad del baneo como la pareja
  `(id, rule_id)`. Hacer el `id` compuesto obligaría a tocar el parseo de `unban_handler`, el
  mensaje de confirmación del borrado y dos requisitos ya especificados, sin arreglar nada que
  el frontend no pueda arreglar por su lado.
- **No se agrupan los baneos de una misma IP en una sola fila.** La spec los considera baneos
  distintos (`rule_id` distinto, con duración y escalado propios) y la tabla muestra la regla
  de cada uno.
- No se toca el paginador de `CustomTable` ni su `pageSize`, que quedaron correctos en
  `admin-ui-fixes`.
- No se toca el backend, ni el pipeline WAF/Jail, ni las reglas, ni los perfiles.

## Impact

- `frontend/src/components/custom_table.tsx` — prop opcional `rowKey`
- `frontend/src/pages/admin/bans_page.tsx` — clave de fila de la partición
- `frontend/src/components/custom_table.test.tsx` — RED de la fila con `id` repetido
- Spec `ban-manager` — requisito nuevo sobre la identidad de fila
- Sin cambios en backend, migraciones, base de datos ni infraestructura

## Límite de la evidencia

La **precondición** está medida en producción: 4 IPs con dos baneos, o sea ocho filas sobre
cuatro `id`. El crecimiento acumulado se atribuye a la `key` repetida, que es un error de
React por definición. Lo que **todavía no está probado** es que jsdom reproduzca el DOM
acumulado; si el RED no lo reproduce, el test afirmará directamente lo que sí es
responsabilidad nuestra —que la clave de fila derivada es única por fila y que React no emite
el aviso de claves duplicadas—. El RED decide cuál de las dos formas toma el test: ninguna se
da por buena sin ejecutarla.
