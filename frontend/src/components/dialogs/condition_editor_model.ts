import type {
  Collection,
  Condition,
  EditableConditionOperator,
  ProjectableFields,
} from "@/models/rule";
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
 * montados.
 *
 * `operator` es la elección de la fila y sólo admite los siete operadores
 * editables: `@truthy` queda fuera porque la colección booleana `is_tor` fija su
 * operador al serializar y su fila no ofrece selector. El valor por defecto de
 * una fila nueva es `@rx`, el mismo que la API asume cuando el campo se omite.
 */
export interface DraftCondition {
  uid: number;
  collection: Collection;
  key: string;
  transformations: string;
  operator: EditableConditionOperator;
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
    operator: editableOperator(condition.operator),
    pattern: condition.pattern,
  };
}

/**
 * El operador editable de una condición recibida, con `@rx` por defecto.
 *
 * El operador recibido se copia tal cual cuando es uno de los siete editables y
 * cae en `@rx` —el valor por defecto, que es también el que la API asume cuando
 * el campo se omite— cuando falta o cuando vale `@truthy`. `@truthy` es
 * exclusivo de la colección booleana `is_tor`, cuya fila fija su operador al
 * serializar y no lo elige: el valor del borrador no se usa allí.
 */
function editableOperator(
  operator: Condition["operator"],
): EditableConditionOperator {
  return operator === undefined || operator === "@truthy" ? "@rx" : operator;
}

/**
 * ¿El operador exige un patrón no vacío?
 *
 * Falso **sólo** para `@unconditionalMatch`, cuyo contrato es el patrón vacío
 * —dejar el anterior produciría un `400`—: es el único de los siete editables que
 * casa sin comparar contra nada. El resto —`@rx`, `@contains`, `@streq`,
 * `@beginsWith`, `@endsWith` y `@within`— compara contra el patrón y lo exige no
 * vacío, así que lo requiere.
 *
 * Vive aquí, en el modelo puro, porque lo consumen los dos lados del editor: el
 * componente para decidir si muestra el campo de patrón y el diálogo para la
 * validación de fila al guardar.
 */
export function operatorRequiresPattern(
  operator: EditableConditionOperator,
): boolean {
  return operator !== "@unconditionalMatch";
}

/**
 * Serializa una fila a la forma que espera la API.
 *
 * `key` viaja con la clave sólo en las colecciones que la exigen y como `null` en
 * el resto —nunca ausente—. El operador es el elegido en la fila (`row.operator`)
 * salvo en la booleana `is_tor`, cuya fila no ofrece elección: fija `@truthy`.
 * Una fila `is_tor` no lleva patrón ni transformaciones —no tienen sentido para un
 * booleano—, así que viajan vacíos.
 *
 * El patrón viaja tal cual se escribió, salvo en `@unconditionalMatch`: ahí el
 * contrato exige el patrón **vacío**, así que se envía vacío aunque el borrador
 * conserve el anterior —el campo está oculto— para que volver a otro operador lo
 * recupere.
 */
export function serializeCondition(row: DraftCondition): Condition {
  if (row.collection === "is_tor") {
    return {
      collection: "is_tor",
      key: null,
      transformations: "",
      operator: "@truthy",
      pattern: "",
    };
  }
  return {
    collection: row.collection,
    key: collectionRequiresKey(row.collection) ? row.key : null,
    transformations: row.transformations,
    operator: row.operator,
    pattern: operatorRequiresPattern(row.operator) ? row.pattern : "",
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
  if (typeof values.is_tor === "boolean") {
    fields.is_tor = values.is_tor;
  }
  return fields;
}

/**
 * Siembra la lista de condiciones con la proyección de los 14 campos rellenos y
 * la columna `is_tor`.
 *
 * Es el estado al pasar a avanzado: así el cambio de modo no pierde ninguna
 * restricción y el punto de partida es el conjunto que el backend ya venía
 * evaluando, en orden canónico —la condición `is_tor`/`@truthy` al final—.
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
    operator: "@rx",
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
  // Identificadores de las filas cuyo operador exige patrón y lo tienen vacío.
  // Hermano de `invalidKeyUids`: se puebla al intentar guardar y el diálogo los
  // limpia a la vez.
  invalidPatternUids: number[];
  disabled: boolean;
  confirmOpen: boolean;
  t: (key: string) => string;
  onModeChange: (mode: ConditionMode) => void;
  onConfirmOpenChange: (open: boolean) => void;
  onConfirmSwitchToSimple: () => void;
  onAdd: () => void;
  onRemove: (uid: number) => void;
  onUpdate: (uid: number, patch: Partial<DraftCondition>) => void;
  onOperatorChange: (uid: number, operator: EditableConditionOperator) => void;
  onInsertTransformation: (uid: number, token: string) => void;
}
