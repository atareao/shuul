const BASE_URL = import.meta.env.VITE_BASE_URL;
const VERSION = import.meta.env.VITE_VERSION;

/**
 * Compone el título de la aplicación con la versión entre paréntesis.
 *
 * Sin versión —ausente, vacía o solo espacios— devuelve el nombre a secas, para
 * no imprimir nunca la cadena `undefined`.
 */
function formatTitle(version: string | undefined): string {
  const trimmed = version?.trim();
  if (!trimmed) {
    return "Shuul";
  }
  return `Shuul (${trimmed})`;
}

const TITLE = formatTitle(VERSION);

export { BASE_URL, VERSION, formatTitle, TITLE };
