# Fix Whitelist/Blacklist Response Format

## Intent
Corregir el formato de respuesta de los endpoints `GET /api/v1/whitelist` y `GET /api/v1/blacklist` para que devuelvan un `PagedResponse` en lugar de un array JSON plano, haciendo que los datos se muestren correctamente en el frontend.

## Scope
Dos archivos backend:
- `backend/src/http/whitelist.rs` — función `list_whitelist`
- `backend/src/http/blacklist.rs` — función `list_blacklist`

## Impacto
- **Backend**: Cambio mínimo — el `GET` devuelve `{status, data, pagination}` en vez de `[...]`.
- **Frontend**: Ningún cambio — el `CustomTable.fetchData` ya espera este formato.
- **API**: Breaking change mínimo — el endpoint ahora devuelve objeto con `data` anidado. Los clientes que consumían el array directamente se rompen, pero no hay clientes externos documentados.
- **Tests**: Los tests existentes de `list_whitelist` / `list_blacklist` deben actualizarse si parsean la respuesta como array.

## Estado actual
`list_whitelist` y `list_blacklist` devuelven:
```rust
Ok(Json(entries))  // → devuelve [{...}, {...}] (array plano)
```

El frontend (`CustomTable.fetchData`) espera:
```typescript
responseJson.status === 200 && responseJson.data  // → necesita {status, data, pagination}
```

Como el array no tiene propiedad `.status`, la condición falla y la tabla se muestra vacía.

## Estado deseado
```rust
let count = entries.len() as i64;
Ok(PagedResponse::new(
    StatusCode::OK,
    "Whitelist entries",
    Data::Some(serde_json::to_value(entries)?),
    Pagination {
        page: 1,
        limit: count.max(1) as u32,
        pages: if count > 0 { 1 } else { 0 },
        records: count,
        prev: None,
        next: None,
    },
))
```

## Contratos

### Antes
`GET /api/v1/whitelist` → `200 OK` + `Content-Type: application/json`
```json
[{"id":1,"entry_type":"ip","value":"10.0.0.1","description":"","created_at":"..."}]
```

### Después
`GET /api/v1/whitelist` → `200 OK` + `Content-Type: application/json`
```json
{
  "status": 200,
  "message": "Whitelist entries",
  "data": [{"id":1,"entry_type":"ip","value":"10.0.0.1","description":"","created_at":"..."}],
  "pagination": {
    "page": 1,
    "limit": 1,
    "pages": 1,
    "records": 1,
    "prev": null,
    "next": null
  }
}
```