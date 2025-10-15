import { BASE_URL } from '@/constants';
import type Response from '@/models/response';
import type { Dictionary } from '@/common/types';


export const loadData = async <T>(
    endpoint: string,
    params?: Dictionary<string | number | boolean | (string | number | boolean)[]>
): Promise<Response<T>> => {
    console.log("Loading data");

    const basePath = `${BASE_URL}/api/v1/${endpoint}`;
    const url = new URL(basePath);

    if (params) {
        // Añadir parámetros de consulta de forma segura
        for (const key in params) {
            const value = params[key];
            if (value !== undefined && value !== null) {
                if (Array.isArray(value)) {
                    // Manejar arrays, si es necesario (ej. para filtros múltiples)
                    value.forEach(item => url.searchParams.append(key, String(item)));
                } else {
                    url.searchParams.append(key, String(value));
                }
            }
        }
    }

    console.log("Fetching URL:", url.toString());

    try {
        const response = await fetch(url.toString(), {
            method: 'GET',
            // El Content-Type es típicamente innecesario para un GET sin cuerpo,
            // pero lo dejo si es una convención de tu API.
            headers: {
                'Content-Type': 'application/json',
            },
        });

        // 2. Manejar respuestas no-OK (ej: 404, 500)
        if (!response.ok) {
            let errorBody: { message?: string } = {};
            try {
                // Intentar leer el cuerpo del error si está en formato JSON
                errorBody = await response.json();
            } catch (e) {
                // Si falla al parsear, ignorar y usar un mensaje por defecto
            }

            return {
                status: response.status,
                message: errorBody.message || `Error HTTP: ${response.status} - ${response.statusText}`
            };
        }

        // 3. Manejar respuestas OK (2xx)
        // La respuesta de la API es un JSON que mapea a T.
        return await response.json();

    } catch (error) {
        // 4. Manejar errores de red o de la función fetch (ej: CORS, sin conexión)
        const msg = error instanceof Error ? error.message : String(error);
        console.error('Network Error or Fetch Failure:', msg, error);

        return {
            status: 500,
            message: `Network or Unknown Error: ${msg}`
        };
    }
};
