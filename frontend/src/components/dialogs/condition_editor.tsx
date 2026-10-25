import { Typography, Flex, Input, Select, Button, Segmented, Popconfirm } from "antd";
import type { Collection, EditableConditionOperator } from "@/models/rule";
import {
  collectionRequiresKey,
  CONDITION_COLLECTIONS,
  CONDITION_OPERATORS,
} from "@/models/rule";
import { CANONICAL_TRANSFORMATIONS } from "./rule_transformations";
import type { ConditionMode, ConditionEditorProps, DraftCondition } from "./condition_editor_model";
import {
  ADVANCED_NOTE_KEY,
  LABEL_WIDTH,
  operatorRequiresPattern,
} from "./condition_editor_model";

const { Text } = Typography;

/**
 * Las opciones del selector de operador, en el orden del catálogo.
 *
 * Es una constante de módulo —el catálogo `CONDITION_OPERATORS` es inmutable— para
 * que el array de `{ value, label }` no se reconstruya en cada render de cada
 * fila.
 */
const CONDITION_OPERATOR_OPTIONS: {
  value: EditableConditionOperator;
  label: EditableConditionOperator;
}[] = CONDITION_OPERATORS.map((operator) => ({
  value: operator,
  label: operator,
}));

/**
 * Editor de condiciones del diálogo de regla: el conmutador simple/avanzado, la
 * nota del modo avanzado y la lista ordenada de filas, con sus chips de
 * transformaciones.
 *
 * Es **presentacional**: recibe todo por props y no guarda estado propio de
 * negocio. El modo, las filas y los `uid` pendientes viven en el diálogo.
 */
export default function ConditionEditor({
  mode,
  conditions,
  invalidKeyUids,
  invalidPatternUids,
  disabled,
  confirmOpen,
  t,
  onModeChange,
  onConfirmOpenChange,
  onConfirmSwitchToSimple,
  onAdd,
  onRemove,
  onUpdate,
  onOperatorChange,
  onInsertTransformation,
}: ConditionEditorProps) {
  /**
   * Una fila del editor de condiciones.
   *
   * La clave sólo se ofrece —y se exige— en las colecciones que la resuelven por
   * nombre; en el resto no aparece. Cada fila de texto ofrece un selector de
   * operador con los siete del catálogo —`@rx` por defecto—, y el campo de patrón
   * se oculta cuando el operador elegido no lo exige (`@unconditionalMatch`). La
   * booleana `is_tor` es la excepción: fija `@truthy` **sin selector** —ofrecerlo
   * insinuaría una elección que el backend rechaza— y omite patrón, clave y
   * transformaciones. La fila se envuelve en un `listitem` para que la lista
   * entera sea anunciable como un conjunto ordenado.
   */
  const renderConditionRow = (row: DraftCondition) => {
    // La colección booleana `is_tor` no se compara contra un patrón ni se
    // resuelve por nombre: su fila fija `@truthy` y omite patrón, clave y
    // transformaciones, que no tendrían sentido.
    const isTor = row.collection === "is_tor";
    return (
      <div role="listitem" key={row.uid}>
        <Flex
          vertical
          gap="small"
          style={{
            border: "1px solid var(--color-border)",
            borderRadius: 6,
            padding: 12,
            marginBottom: 8,
          }}
        >
          <Flex align="center" gap="small">
            <Text style={{ width: LABEL_WIDTH, flexShrink: 0 }}>
              {t("Collection")}
            </Text>
            <Select<Collection>
              style={{ width: "100%" }}
              aria-label={t("Collection")}
              value={row.collection}
              showSearch
              optionFilterProp="label"
              onChange={(value) =>
                onUpdate(row.uid, {
                  collection: value,
                  key: collectionRequiresKey(value) ? row.key : "",
                })
              }
              disabled={disabled}
              options={CONDITION_COLLECTIONS.map((collection) => ({
                value: collection,
                label: collection,
              }))}
            />
          </Flex>
          {collectionRequiresKey(row.collection) && (
            <Flex align="start" gap="small">
              <Text style={{ width: LABEL_WIDTH, flexShrink: 0, paddingTop: 4 }}>
                {t("Key")}
              </Text>
              <Flex vertical gap={4} style={{ width: "100%" }}>
                <Input
                  style={{ width: "100%" }}
                  aria-label={t("Key")}
                  aria-invalid={invalidKeyUids.includes(row.uid)}
                  status={invalidKeyUids.includes(row.uid) ? "error" : undefined}
                  value={row.key}
                  onChange={(e) => onUpdate(row.uid, { key: e.target.value })}
                  disabled={disabled}
                />
                {invalidKeyUids.includes(row.uid) && (
                  <Text type="danger" role="alert">{t("This key is required")}</Text>
                )}
              </Flex>
            </Flex>
          )}
          {/*
            El patrón se oculta cuando el operador no lo exige
            (`@unconditionalMatch`): su contrato es el patrón vacío y dejar el
            anterior en la fila produciría un `400` al guardar. El borrador lo
            conserva para que volver a otro operador lo recupere.
          */}
          {!isTor && operatorRequiresPattern(row.operator) && (
            <Flex align="start" gap="small">
              <Text style={{ width: LABEL_WIDTH, flexShrink: 0, paddingTop: 4 }}>
                {t("Pattern")}
              </Text>
              <Flex vertical gap={4} style={{ width: "100%" }}>
                <Input
                  style={{ width: "100%" }}
                  aria-label={t("Pattern")}
                  aria-invalid={invalidPatternUids.includes(row.uid)}
                  status={
                    invalidPatternUids.includes(row.uid) ? "error" : undefined
                  }
                  value={row.pattern}
                  onChange={(e) => onUpdate(row.uid, { pattern: e.target.value })}
                  disabled={disabled}
                />
                {invalidPatternUids.includes(row.uid) && (
                  <Text type="danger" role="alert">
                    {t("This pattern is required")}
                  </Text>
                )}
              </Flex>
            </Flex>
          )}
          {!isTor && (
            <Flex vertical gap={4}>
              <Flex align="center" gap="small">
                <Text style={{ width: LABEL_WIDTH, flexShrink: 0 }}>
                  {t("Transformations")}
                </Text>
                <Input
                  style={{ width: "100%" }}
                  aria-label={t("Transformations")}
                  value={row.transformations}
                  placeholder={t("Transformations")}
                  onChange={(e) =>
                    onUpdate(row.uid, { transformations: e.target.value })
                  }
                  disabled={disabled}
                />
              </Flex>
              <Flex
                wrap
                gap="small"
                role="group"
                aria-label={t("Transformations")}
                style={{ marginLeft: LABEL_WIDTH + 8 }}
              >
                {CANONICAL_TRANSFORMATIONS.map((token) => (
                  <Button
                    key={token}
                    size="small"
                    disabled={disabled}
                    onClick={() => onInsertTransformation(row.uid, token)}
                  >
                    {token}
                  </Button>
                ))}
              </Flex>
            </Flex>
          )}
          {/*
            La booleana `is_tor` conserva su operador **fijo** como texto: ofrecer
            el selector insinuaría una elección que el backend rechaza con `400`
            —`@truthy` es exclusivo de `is_tor`—. El resto de filas eligen el suyo
            entre los siete del catálogo, con `@rx` por defecto.
          */}
          {isTor ? (
            <Flex align="center" gap="small">
              <Text style={{ width: LABEL_WIDTH, flexShrink: 0 }}>
                {t("Operator")}
              </Text>
              <Text>@truthy</Text>
            </Flex>
          ) : (
            <Flex align="center" gap="small">
              <Text style={{ width: LABEL_WIDTH, flexShrink: 0 }}>
                {t("Operator")}
              </Text>
              <Select<EditableConditionOperator>
                style={{ width: "100%" }}
                aria-label={t("Operator")}
                value={row.operator}
                onChange={(value) => onOperatorChange(row.uid, value)}
                disabled={disabled}
                // El catálogo es de siete opciones cortas: se rinde completo y
                // **sin** la lista virtualizada de antd a propósito, para que el
                // desplegable sea determinista. La virtualización no mide alturas
                // en jsdom y monta sólo un subconjunto de las opciones, así que
                // con ella el test no podría enumerar las siete; con
                // `virtual={false}` las siete están siempre en el DOM y la opción
                // se localiza por su `title`.
                virtual={false}
                options={CONDITION_OPERATOR_OPTIONS}
              />
            </Flex>
          )}
          <Button
            size="small"
            disabled={disabled}
            onClick={() => onRemove(row.uid)}
            style={{ alignSelf: "flex-end" }}
          >
            {t("Remove")}
          </Button>
        </Flex>
      </div>
    );
  };

  const modeSwitcher = (
    <Segmented<ConditionMode>
      aria-label={t("Condition mode")}
      value={mode}
      onChange={onModeChange}
      options={[
        { label: t("Simple"), value: "simple" },
        { label: t("Advanced"), value: "advanced" },
      ]}
      disabled={disabled}
    />
  );

  return (
    <Flex vertical gap="middle" style={{ paddingTop: 16 }}>
      {/*
        El conmutador es un `Segmented` de antd, el control acordado con el
        usuario. Su `input[type=radio]` interno lleva `pointer-events: none`
        (estilo de antd), así que los tests accionan la etiqueta visible, que es
        la que recibe el puntero. El nombre accesible del `radio` sigue siendo
        el texto visible de la opción.
      */}
      {mode === "advanced" ? (
        // El `Popconfirm` sólo existe en avanzado —es donde tiene sentido volver a
        // simple— y se **desmonta** al confirmar. Montarlo siempre obligaría a
        // esperar a que terminase su animación de salida para que su botón `Ok`
        // dejase de convivir con el del diálogo; al desmontarlo, ambos estados son
        // disjuntos. Su apertura la gobierna `confirmOpen` desde
        // `handleConditionModeChange`, y su cierre se atiende en
        // `onConfirmOpenChange`: los disparadores `click` sobre los radios
        // se ignoran a propósito.
        <Popconfirm
          title={t("Switch to simple mode")}
          description={t("The conditions list will be discarded. Continue?")}
          open={confirmOpen}
          onOpenChange={onConfirmOpenChange}
          onConfirm={onConfirmSwitchToSimple}
          okText={t("Ok")}
          cancelText={t("Cancel")}
        >
          {/* `Popconfirm` exige un único hijo que acepte ref y eventos. */}
          <span style={{ display: "inline-block" }}>{modeSwitcher}</span>
        </Popconfirm>
      ) : (
        <span style={{ display: "inline-block" }}>{modeSwitcher}</span>
      )}
      {mode === "advanced" && (
        <>
          <Text type="secondary">{t(ADVANCED_NOTE_KEY)}</Text>
          <div role="list" aria-label={t("Conditions")}>
            {conditions.map(renderConditionRow)}
          </div>
          <Button
            onClick={onAdd}
            disabled={disabled}
            style={{ alignSelf: "flex-start" }}
          >
            {t("Add condition")}
          </Button>
        </>
      )}
    </Flex>
  );
}
