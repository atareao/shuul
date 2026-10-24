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

/** Cuántas entradas rechazadas se nombran antes de resumir el resto. */
const MAX_ENUMERATED_REJECTIONS = 3;

/** Marcador para una entrada rechazada cuyo nombre no es utilizable. */
const UNNAMED_REJECTION = "unnamed rule";

/**
 * Recuento de importadas, tolerante con un backend que no mande un número.
 *
 * `Number.isFinite` descarta `undefined`, `NaN` e `Infinity` de una vez: sin él
 * el aviso diría `Imported undefined rules` o `Imported NaN rules`.
 */
function importedCount(imported: unknown): number {
  return typeof imported === "number" && Number.isFinite(imported)
    ? imported
    : 0;
}

/**
 * Nombra una entrada rechazada, con su motivo cuando lo trae.
 *
 * Ni el nombre ni el motivo son de fiar —llegan de un JSON ajeno—, así que un
 * valor ausente, vacío o que no sea cadena cae en un marcador legible en vez de
 * imprimir `undefined`.
 */
function describeRejectedRule(entry: RejectedRule | null | undefined): string {
  const rawName = entry?.name;
  const name =
    typeof rawName === "string" && rawName.trim() !== ""
      ? rawName
      : UNNAMED_REJECTION;
  const reason = entry?.reason;

  return typeof reason === "string" && reason.trim() !== ""
    ? `${name} (${reason})`
    : name;
}

/**
 * Traduce el resultado de una importación al aviso que hay que mostrar.
 *
 * Sin rechazos (lista ausente o vacía) el camino es el de éxito y el texto es el
 * de siempre: `Imported N rules`. Con rechazos, el texto cuenta cuántas se
 * importaron y nombra cada una de las descartadas, con su motivo cuando viene.
 *
 * La enumeración está acotada: un fichero con muchas entradas inválidas pintaría
 * un toast interminable, así que sólo se nombran las
 * `MAX_ENUMERATED_REJECTIONS` primeras y el resto se resume como `… and N more`.
 * Con ese número de rechazadas o menos, el texto es el de siempre.
 */
export function describeRuleImport(result: RuleImportResult): RuleImportNotice {
  const imported = importedCount(result.imported);
  const rejected = Array.isArray(result.rejected) ? result.rejected : [];
  if (rejected.length === 0) {
    return { kind: "success", text: `Imported ${imported} rules` };
  }

  const enumerated = rejected
    .slice(0, MAX_ENUMERATED_REJECTIONS)
    .map(describeRejectedRule)
    .join(", ");
  const remaining = rejected.length - MAX_ENUMERATED_REJECTIONS;
  const names =
    remaining > 0 ? `${enumerated}, … and ${remaining} more` : enumerated;

  return {
    kind: "warning",
    text: `Imported ${imported} rules, ${rejected.length} rejected: ${names}`,
  };
}
