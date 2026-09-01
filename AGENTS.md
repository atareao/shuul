# Reglas específicas del proyecto Shuul

## TypeScript

### `debounce` siempre debe tiparse con `.cancel()`

La función `debounce` en `frontend/src/common/utils.ts` devuelve una función con un método `.cancel()`. Si no se tipa correctamente, TypeScript se queja al llamar a `.cancel()` en `componentWillUnmount`.

**Solución:** Usar la interfaz `DebouncedFn<T>` exportada desde `utils.ts`:

```typescript
import type { DebouncedFn } from '@/common/utils';

// ✅ BIEN
private debouncedSetFilter: DebouncedFn<(key: string, value: string) => void>;

// ❌ MAL (TypeScript error TS2339: Property 'cancel' does not exist)
private debouncedSetFilter: (key: string, value: string) => void;
```

### `loadData` con query params: usar `Map`, no embeker en URL

`loadData(endpoint, paramsMap)` construye la URL con `BASE_URL` y serializa el `Map` como query params con `URLSearchParams`. **NUNCA** embebas query params en el string del endpoint:

```typescript
// ❌ MAL: query params en el endpoint
loadData(`requests/evolution?unit=${unit}&last=${last}`)

// ✅ BIEN: query params como Map
loadData("requests/evolution", new Map([["unit", unit], ["last", last.toString()]]))
```

### Llamadas API secuenciales: usar `Promise.all()`

Cuando un componente necesita cargar datos de múltiples endpoints en `componentDidMount`, **NUNCA** uses `await` secuenciales. Usa `Promise.all()` para paralelizar:

```typescript
// ❌ MAL: ~1.5s de carga
const a = await loadData(...);
const b = await loadData(...);
const c = await loadData(...);

// ✅ BIEN: ~300ms (lo que dure la más lenta)
const [a, b, c] = await Promise.all([
    loadData(...),
    loadData(...),
    loadData(...),
]);
```

### `componentDidUpdate`: early return para cambios irrelevantes

Cuando `loading` o `items` cambian en un `componentDidUpdate`, no se necesita hacer nada más (es el resultado de un fetch). Añadir un early return al inicio:

```typescript
componentDidUpdate = async (prevProps, prevState) => {
    // Early return: ignorar cambios solo en loading o items
    if (prevState.loading !== this.state.loading || prevState.items !== this.state.items) {
        return;
    }
    // ... resto de la lógica
}
```