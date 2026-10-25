/**
 * Los dieciocho nombres canónicos de transformación, **en el orden de la
 * especificación** (`urlDecode`, `urlDecodeUni`, `lowercase`…) —el mismo que
 * sirve el backend—.
 *
 * El orden importa: la pipeline se aplica de izquierda a derecha y no es
 * conmutativa (`urlDecodeUni,lowercase` no es `lowercase,urlDecodeUni`), así que
 * esta lista es sólo el orden en que se ofrecen los chips, no el orden en que se
 * aplican: el que manda es el orden en que se pulsan.
 *
 * El backend **rechaza** cualquier token que no sea exactamente uno de estos,
 * literal y sensible a mayúsculas (`urldecodeuni` no vale), con un `400` que
 * nombra el token. Por eso se ofrecen como chips en vez de dejar que se escriban
 * a mano: el camino común —pulsar— no puede producir un token no canónico, y el
 * parser estricto sólo lo ve quien escribe contra la API.
 */
export const CANONICAL_TRANSFORMATIONS = [
  "urlDecode",
  "urlDecodeUni",
  "lowercase",
  "uppercase",
  "removeNulls",
  "replaceNulls",
  "compressWhitespace",
  "removeWhitespace",
  "normalizePath",
  "normalizePathWin",
  "htmlEntityDecode",
  "jsDecode",
  "cssDecode",
  "escapeSeqDecode",
  "base64Decode",
  "cmdLine",
  "trim",
  "length",
] as const;
