export default interface Rule {
  id: number;
  name: string;
  description: string;
  mode: string;
  weight?: number;
  allow?: boolean;
  pipeline?: string;
  transformations?: string;
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
  conditions?: Condition[];
  created_at?: Date;
  updated_at?: Date;
}

/**
 * Una colección del conjunto ordenado de condiciones que evalúa el backend.
 *
 * Son las 19 de F2b-2a: las 14 de campo —una por columna restringible del modo
 * simple— más las cinco que introduce el editor avanzado. Tres de ellas —la
 * cookie, la cabecera y el argumento del query— identifican un valor **por
 * nombre**, así que exigen una clave; las otras dos (`request_basename` y
 * `request_extension`) se resuelven del camino y no la llevan.
 *
 * El tipo es la unión literal, no `string`, para que el compilador vigile los
 * catálogos y el editor: una colección inventada no compila.
 */
export type Collection =
  | "ip_address"
  | "protocol"
  | "fqdn"
  | "path"
  | "query"
  | "city_name"
  | "country_name"
  | "country_code"
  | "user_agent"
  | "method"
  | "referer"
  | "content_type"
  | "accept_language"
  | "x_request_id"
  | "request_cookies"
  | "request_headers"
  | "args_get"
  | "request_basename"
  | "request_extension";

/**
 * Las 14 colecciones de campo, **en el orden canónico de evaluación**.
 *
 * El orden no es decorativo: es el orden en que el backend recorre los campos al
 * proyectar una regla simple y, por tanto, el orden en que se emiten las
 * condiciones. `projectConditions()` lo respeta para que cambiar de modo no
 * reordene el conjunto persistido.
 */
export const FIELD_COLLECTIONS: readonly Collection[] = [
  "ip_address",
  "protocol",
  "fqdn",
  "path",
  "query",
  "city_name",
  "country_name",
  "country_code",
  "user_agent",
  "method",
  "referer",
  "content_type",
  "accept_language",
  "x_request_id",
];

/**
 * Las 19 colecciones, con las 14 de campo primero —en orden canónico— y las cinco
 * de F2b-2a después.
 *
 * Es el catálogo que ofrece el selector del editor: un orden estable y predecible
 * facilita localizar una colección a mano, aunque el test evite elegir por clic
 * las que antd virtualiza al final de la lista.
 */
export const CONDITION_COLLECTIONS: readonly Collection[] = [
  ...FIELD_COLLECTIONS,
  "request_cookies",
  "request_headers",
  "args_get",
  "request_basename",
  "request_extension",
];

/**
 * Las tres colecciones cuyo valor se resuelve por nombre y, por tanto, exigen
 * clave. Vive fuera de `collectionRequiresKey` para poder reutilizarse en
 * validaciones sin volver a enumerarlas.
 */
const KEYED_COLLECTIONS: readonly Collection[] = [
  "request_cookies",
  "request_headers",
  "args_get",
];

/**
 * ¿La colección exige clave?
 *
 * Verdadero exactamente para `request_cookies`, `request_headers` y `args_get`:
 * son las únicas cuyo valor se identifica por el nombre de la cookie, de la
 * cabecera o del argumento del query. El editor lo usa para mostrar —y exigir—
 * el Input «Key» sólo donde tiene sentido, y el serializador lo usa para decidir
 * si `key` viaja con la clave o como `null`.
 */
export function collectionRequiresKey(collection: Collection): boolean {
  return KEYED_COLLECTIONS.includes(collection);
}

/**
 * Una condición ya compilada, en su forma serializada: la misma que lee y escribe
 * la API.
 *
 * `key` es `null` —no `undefined`— en las colecciones sin clave, de modo que el
 * cuerpo JSON lleva la clave **presente** y nula, que es lo que el backend espera
 * deserializar. `operator` es opcional porque la API lo omite cuando vale `@rx`
 * —hoy su único operador—, y el editor lo fija siempre a `@rx`. F2c lo ampliará
 * a una unión de operadores.
 */
export interface Condition {
  collection: Collection;
  key?: string | null;
  transformations: string;
  operator?: "@rx";
  pattern: string;
}

/**
 * La forma proyectable de una regla: los 14 campos de campo más la pipeline de
 * transformaciones.
 *
 * Se declara como `Partial<Record<…>>` y no como `Rule` para que la proyección
 * pueda invocarse también con los valores sueltos del formulario, que son
 * `Record<string, unknown>`. `is_tor` **no** forma parte: no es una colección de
 * campo y nunca proyecta.
 */
export type ProjectableFields = Partial<Record<Collection | "transformations", string>>;

/**
 * Proyecta los 14 campos planos al conjunto ordenado de condiciones, con el
 * **mismo criterio que la API**.
 *
 * Recorre las 14 colecciones de campo en su orden canónico —no las claves del
 * objeto, que vienen en el orden en que se escribieron— y emite una condición por
 * cada campo con patrón no vacío tras `trim`. El patrón se copia **tal cual** se
 * escribió: el `trim` sólo decide si el campo participa, no recorta el valor.
 *
 * `is_tor` queda fuera por construcción: no es ninguna de las 14 colecciones, así
 * que una regla que sólo restrinja Tor proyecta el conjunto vacío. Es lo correcto:
 * el predicado de Tor se combina aparte en el backend, no como condición.
 *
 * La pipeline de transformaciones se copia en cada condición, y la cadena vacía
 * es el valor por defecto cuando la regla no declara ninguna.
 */
export function projectConditions(rule: ProjectableFields): Condition[] {
  const transformations = rule.transformations ?? "";
  const conditions: Condition[] = [];
  for (const collection of FIELD_COLLECTIONS) {
    const pattern = rule[collection];
    if (typeof pattern !== "string" || pattern.trim() === "") {
      continue;
    }
    conditions.push({
      collection,
      key: null,
      transformations,
      operator: "@rx",
      pattern,
    });
  }
  return conditions;
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
