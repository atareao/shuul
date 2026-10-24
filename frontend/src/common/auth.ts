import { BASE_URL } from "@/constants";

/**
 * Arranca el flujo SSO: saca al navegador de la aplicación hacia el endpoint del
 * backend, que redirige al proveedor OIDC. Es el único sitio del frontend que
 * compone esa URL.
 */
export function startSsoLogin(): void {
  window.location.href = `${BASE_URL}/api/v1/auth/sso`;
}
