import { BASE_URL } from "@/constants";
import type Response from "@/models/response";

/** Valores escalares admitidos como parámetro de consulta. */
export type QueryParamValue = string | number | boolean;

/** Consulta opcional: cada clave admite un valor o una lista de valores. */
export type QueryParams = Map<string, QueryParamValue | QueryParamValue[]>;

export const loadData = async <T>(
  endpoint: string,
  params?: QueryParams,
): Promise<Response<T>> => {
  console.log("Loading data");

  const token = localStorage.getItem("token");

  const basePath = `${BASE_URL}/api/v1/${endpoint}`;
  const searchParams = new URLSearchParams();
  if (params) {
    console.log("Params", params);
    // Añadir parámetros de consulta de forma segura
    params.forEach((value, key) => {
      console.log(`Param: ${key} = ${value}`);
      if (value !== undefined && value !== null) {
        if (Array.isArray(value)) {
          // Manejar arrays, si es necesario (ej. para filtros múltiples)
          value.forEach((item) => searchParams.append(key, String(item)));
        } else {
          searchParams.append(key, String(value));
        }
      }
    });
  }
  const queryString = searchParams.toString();
  const url = `${basePath}${queryString ? `?${queryString}` : ""}`;

  console.log("Fetching URL:", url);

  try {
    const response = await fetch(url, {
      method: "GET",
      headers: {
        "Content-Type": "application/json",
        ...(token ? { Authorization: `Bearer ${token}` } : {}),
      },
    });

    // 2. Manejar respuestas no-OK (ej: 404, 500)
    if (!response.ok) {
      if (response.status === 401) {
        window.location.href = "/login";
        throw new Error("Session expired");
      }
      let errorBody: { message?: string } = {};
      try {
        // Intentar leer el cuerpo del error si está en formato JSON
        errorBody = await response.json();
      } catch {
        // Si falla al parsear, ignorar y usar un mensaje por defecto
      }
      const msg =
        errorBody.message ||
        `Error HTTP: ${response.status} - ${response.statusText}`;
      console.error(`API Error Response: ${msg}`);
      return {
        status: response.status,
        message: msg,
      };
    }

    // 3. Manejar respuestas OK (2xx)
    // La respuesta de la API es un JSON que mapea a T.
    return await response.json();
  } catch (error) {
    // 4. Manejar errores de red o de la función fetch (ej: CORS, sin conexión)
    const msg = error instanceof Error ? error.message : String(error);
    console.error("Network Error or Fetch Failure:", msg, error);

    return {
      status: 500,
      message: `Network or Unknown Error: ${msg}`,
    };
  }
};

export const mapsEqual = (
  map1: Map<string, string>,
  map2: Map<string, string>,
): boolean => {
  // 1. Si el tamaño es diferente, son distintos.
  if (map1.size !== map2.size) {
    return false;
  }

  // 2. Itera sobre el primer mapa y compara los valores en el segundo.
  for (const [key, val] of map1.entries()) {
    // Si la clave no existe o el valor es diferente, son distintos.
    if (val !== map2.get(key)) {
      return false;
    }
  }

  // 3. Si la iteración termina, son iguales.
  return true;
};

export const toCapital = (s: string): string => {
  return s
    .toLowerCase()
    .split(" ")
    .map((word) => word.charAt(0).toUpperCase() + word.substring(1))
    .join(" ");
};

/**
 * Firma genérica de cualquier función. Se usa como restricción de `T` para
 * poder tipar los argumentos con `Parameters<T>` en los puntos de llamada.
 */
export type AnyFunction = (...args: never[]) => void;

export interface DebouncedFn<T extends AnyFunction> {
  (...args: Parameters<T>): void;
  cancel: () => void;
}

export const debounce = <T extends AnyFunction>(
  func: T,
  delay: number,
): DebouncedFn<T> => {
  let timeoutId: ReturnType<typeof setTimeout>;
  const debounced = ((...args: Parameters<T>): void => {
    clearTimeout(timeoutId);
    timeoutId = setTimeout(() => func(...args), delay);
  }) as DebouncedFn<T>;
  debounced.cancel = () => {
    clearTimeout(timeoutId);
  };
  return debounced;
};

/**
 * Obtiene un valor anidado de un objeto de forma segura (ej: 'value.name_es')
 * Esto resuelve el error TS2536.
 *
 * El valor devuelto es `unknown` porque la profundidad y el tipo del dato no
 * se pueden conocer a partir de una ruta en tiempo de ejecución: quien lo use
 * debe estrechar el tipo (`typeof`, comparaciones, `String(...)`, ...).
 */
export const getNestedValue = (obj: unknown, path: string): unknown => {
  // Si el objeto es nulo o indefinido, o la ruta no es válida, devolvemos undefined
  if (!obj || !path) {
    return undefined;
  }

  const pathParts = path.split(".");
  let current: unknown = obj;

  for (const part of pathParts) {
    // Navegamos al siguiente nivel, asegurando que el objeto actual no sea nulo
    if (current && typeof current === "object" && part in current) {
      current = (current as Record<string, unknown>)[part];
    } else {
      // Si el camino se rompe (ej: 'value' es null o 'name_es' no existe en 'value')
      return undefined;
    }
  }
  return current;
};
