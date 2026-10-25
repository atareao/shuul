import type { Collection, Condition, ProjectableFields } from "@/models/rule";
import {
  collectionRequiresKey,
  projectConditions,
  FIELD_COLLECTIONS,
} from "@/models/rule";

/**
 * Clave de traducción de la nota que se muestra en modo avanzado, literal.
 *
 * En avanzado los 14 campos simples se siguen almacenando —y siguen viajando en
 * el cuerpo— pero **no participan del matcheo**: el conjunto persistido es la
 * lista de condiciones. La nota es la única señal visible de esa disociación.
 */
export const ADVANCED_NOTE_KEY =
  "In advanced mode these fields are stored but do not participate in matching";

/**
 * Los dos modos del editor de condiciones.
 *
 * `simple` es el de siempre: el backend proyecta los 14 campos y el cuerpo omite
 * `conditions`. `advanced` envía la lista —aunque esté vacía— y deshabilita los
 * 14 campos, que quedan almacenados sin matchear.
 */
export type ConditionMode = "simple" | "advanced";

/**
 * Una fila del editor de condiciones, con un identificador estable.
 *
 * El `uid` no viaja a la API: es sólo la clave de React, para que reordenar o
 * retirar filas no reasigne el foco ni el identificador de los Inputs ya
 * montados. El operador no se guarda porque hoy es siempre `@rx`, el único que
 * existe: se fija al serializar.
 */
export interface DraftCondition {
  uid: number;
  collection: Collection;
  key: string;
  transformations: string;
  pattern: string;
}

/**
 * Contador de `uid` de fila. Vive a nivel de módulo —no en el estado del
 * componente— para que dos filas nunca compartan identificador aunque se monten
 * y desmonten diálogos distintos.
 */
let nextConditionUid = 1;

/** Normaliza una condición recibida de la API a la forma comparable. */
export function normalizeCondition(condition: Condition) {
  return {
    collection: condition.collection,
    key: condition.key ?? null,
    transformations: condition.transformations,
    operator: condition.operator ?? "@rx",
    pattern: condition.pattern,
  };
}

/**
 * ¿Las condiciones recibidas son exactamente la proyección de los 14 campos?
 *
 * La comparación es campo a campo —`key ?? null`, `operator ?? "@rx"`— y
 * sensible al orden: dos conjuntos con las mismas condiciones en distinto orden
 * difieren. Así la inferencia es sin pérdida: si coinciden, el modo es simple y
 * volver a enviar reproduce el mismo conjunto que el backend ya tenía.
 */
export function conditionsMatch(
  received: Condition[] | undefined,
  projected: Condition[],
): boolean {
  const actual = received ?? [];
  if (actual.length !== projected.length) {
    return false;
  }
  return actual.every((condition, index) => {
    const left = normalizeCondition(condition);
    const right = normalizeCondition(projected[index]);
    return (
      left.collection === right.collection &&
      left.key === right.key &&
      left.transformations === right.transformations &&
      left.operator === right.operator &&
      left.pattern === right.pattern
    );
  });
}

/**
 * Infiere el modo de partida al abrir el diálogo.
 *
 * Una regla nueva —o una sin condiciones— abre en simple. En edición se comparan
 * las condiciones **efectivas** que devuelve la API con la proyección de los 14
 * campos recibidos: coinciden → simple (el conjunto se reproduce proyectando);
 * difieren → avanzado, y la lista arranca con las condiciones recibidas.
 */
export function inferConditionMode(
  item?: ProjectableFields & { conditions?: Condition[] },
): {
  mode: ConditionMode;
  conditions: DraftCondition[];
} {
  if (!item) {
    return { mode: "simple", conditions: [] };
  }
  const received = item.conditions;
  const projected = projectConditions(item);
  if (conditionsMatch(received, projected)) {
    return { mode: "simple", conditions: [] };
  }
  return {
    mode: "advanced",
    conditions: (received ?? []).map(conditionToDraft),
  };
}

/** Convierte una condición recibida en una fila del editor. */
export function conditionToDraft(condition: Condition): DraftCondition {
  return {
    uid: nextConditionUid++,
    collection: condition.collection,
    key: condition.key ?? "",
    transformations: condition.transformations,
    pattern: condition.pattern,
  };
}

/**
 * Serializa una fila a la forma que espera la API.
 *
 * `key` viaja con la clave sólo en las colecciones que la exigen y como `null` en
 * el resto —nunca ausente—, y el operador se fija a `@rx`.
 */
export function serializeCondition(row: DraftCondition): Condition {
  return {
    collection: row.collection,
    key: collectionRequiresKey(row.collection) ? row.key : null,
    transformations: row.transformations,
    operator: "@rx",
    pattern: row.pattern,
  };
}

/**
 * Extrae del formulario los campos proyectables.
 *
 * Los valores del formulario son `Record<string, unknown>`; la proyección
 * espera cadenas. Se copian sólo los campos de texto de las 14 colecciones y la
 * pipeline, de modo que la firma de `projectConditions` no tenga que aflojarse.
 */
export function projectableFieldsFrom(
  values: Record<string, unknown>,
): ProjectableFields {
  const fields: ProjectableFields = {};
  for (const collection of FIELD_COLLECTIONS) {
    const value = values[collection];
    if (typeof value === "string") {
      fields[collection] = value;
    }
  }
  if (typeof values.transformations === "string") {
    fields.transformations = values.transformations;
  }
  return fields;
}

/**
 * Siembra la lista de condiciones con la proyección de los 14 campos rellenos.
 *
 * Es el estado al pasar a avanzado: así el cambio de modo no pierde ninguna
 * restricción y el punto de partida es el conjunto que el backend ya venía
 * evaluando, en orden canónico.
 */
export function seedConditions(
  values: Record<string, unknown>,
): DraftCondition[] {
  return projectConditions(projectableFieldsFrom(values)).map(conditionToDraft);
}

/** Construye una fila vacía con un `uid` fresco, para el botón «Add condition». */
export function newDraftCondition(): DraftCondition {
  return {
    uid: nextConditionUid++,
    collection: FIELD_COLLECTIONS[0],
    key: "",
    transformations: "",
    pattern: "",
  };
}

/**
 * Ancho en píxeles de la columna de etiquetas de cada fila del formulario.
 *
 * Todas las filas (`renderInputRow`, `renderSelectRow` y las escritas a mano en
 * las pestañas) reservan esta anchura fija a la etiqueta para que los campos
 * queden alineados en vertical. El bloque de chips de transformaciones se alinea
 * bajo el campo —no bajo la etiqueta—, así que se desplaza a la derecha esta
 * misma anchura más el hueco (`gap`) del `Flex` de antd.
 */
export const LABEL_WIDTH = 120;

// Props del editor de condiciones.
export interface ConditionEditorProps {
  mode: ConditionMode;
  conditions: DraftCondition[];
  invalidKeyUids: number[];
  disabled: boolean;
  confirmOpen: boolean;
  t: (key: string) => string;
  onModeChange: (mode: ConditionMode) => void;
  onConfirmOpenChange: (open: boolean) => void;
  onConfirmSwitchToSimple: () => void;
  onAdd: () => void;
  onRemove: (uid: number) => void;
  onUpdate: (uid: number, patch: Partial<DraftCondition>) => void;
  onInsertTransformation: (uid: number, token: string) => void;
}
