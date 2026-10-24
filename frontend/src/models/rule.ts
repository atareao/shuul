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

/**
 * Una entrada que el backend no almacenó al importar, con el motivo del rechazo.
 *
 * El motivo es opcional porque el mensaje se compone con tolerancia: si el
 * backend deja de enviarlo, la UI sigue nombrando la regla.
 */
export interface RejectedRule {
  name: string;
  reason?: string;
}

/**
 * Respuesta de `POST /api/v1/rules/import`.
 *
 * `rejected` es opcional a propósito: un backend que sólo devuelva `imported`
 * —o un despliegue antiguo— debe leerse como importación limpia, no como fallo.
 */
export interface RuleImportResult {
  imported: number;
  rejected?: RejectedRule[];
}

/**
 * Aviso ya resuelto para pintar en la UI: `success` cuando no hubo rechazos,
 * `warning` cuando alguno se descartó. El texto es el que ve el usuario.
 */
export interface RuleImportNotice {
  kind: "success" | "warning";
  text: string;
}

/**
 * Traduce el resultado de una importación al aviso que hay que mostrar.
 *
 * Sin rechazos (lista ausente o vacía) el camino es el de éxito y el texto es el
 * de siempre: `Imported N rules`. Con rechazos, el texto cuenta cuántas se
 * importaron y nombra cada una de las descartadas, con su motivo cuando viene.
 */
export function describeRuleImport(result: RuleImportResult): RuleImportNotice {
  const rejected = Array.isArray(result.rejected) ? result.rejected : [];
  if (rejected.length === 0) {
    return { kind: "success", text: `Imported ${result.imported} rules` };
  }

  const names = rejected
    .map((rule) => (rule.reason ? `${rule.name} (${rule.reason})` : rule.name))
    .join(", ");

  return {
    kind: "warning",
    text: `Imported ${result.imported} rules, ${rejected.length} rejected: ${names}`,
  };
}
