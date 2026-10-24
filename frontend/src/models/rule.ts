export default interface Rule {
  id: number;
  name: string;
  description: string;
  mode: string;
  weight?: number;
  allow?: boolean;
  pipeline?: string;
  ip_address?: string;
  protocol?: string;
  fqdn?: string;
  path?: string;
  query?: string;
  city_name?: string;
  country_name?: string;
  country_code?: string;
  user_agent?: string;
  method?: string;
  referer?: string;
  is_tor?: boolean;
  content_type?: string;
  accept_language?: string;
  x_request_id?: string;
  rate_limit_profile_id?: number;
  rate_limit_profile_name?: string;
  active?: number;
  created_at?: Date;
  updated_at?: Date;
}

/**
 * ¿El pipeline indicado es el de Jail?
 *
 * El sistema compara el pipeline de forma literal y sensible a mayúsculas, así
 * que la comparación vive aquí y no repetida en cada vista: un typo en una copia
 * no lo detectaría el compilador. Acepta `unknown` para poder usarse tanto con
 * un `Rule`/`RuleTemplate` como con los valores del formulario, que son
 * `Record<string, unknown>`.
 */
export function isJailPipeline(pipeline: unknown): boolean {
  return pipeline === "jail";
}
