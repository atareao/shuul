/// <reference types="node" />
//
// El `/// <reference types="node" />` de arriba es necesario porque
// `tsconfig.app.json` restringe `types` a `["vite/client"]`, así que `node:fs`
// y `node:url` no resolverían y `npx tsc -b --force` fallaría. La directiva
// trae los tipos de Node solo a este fichero, sin tocar ningún `tsconfig`.

import { describe, it, expect } from "vitest";
import { existsSync, readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

// ─────────────────────────────────────────────────────────────────────────────
// Contrato del delta `admin-sidebar-and-version`, requisito «La versión
// publicada se muestra y nunca aparece como undefined».
//
// El build de producción tiene que declarar la versión publicada, esa
// declaración tiene que coincidir con el fichero que gobierna el versionado del
// proyecto y el versionado tiene que mantenerla en sincronía. Todo se resuelve
// desde la ruta de ESTE test —no desde el directorio de trabajo— y el YAML se
// inspecciona con expresiones regulares sobre el texto, sin dependencias nuevas.
// ─────────────────────────────────────────────────────────────────────────────

// Las dos rutas van en constantes a propósito: Vite reescribe
// `new URL("<literal>", import.meta.url)` como referencia de asset contra el
// servidor de desarrollo (`http://localhost:3000/…`), y `fileURLToPath`
// rechazaría esa URL. Con el literal fuera de la llamada, la URL es la del
// fichero real y las rutas se resuelven desde la ruta de ESTE test, no desde el
// directorio de trabajo.
const REL_ENV_PRODUCTION = "../.env.production";
const REL_VAMPUS = "../../.vampus.yml";

/** `frontend/.env.production`, el modo de producción del build. */
const ENV_PRODUCTION_PATH = fileURLToPath(
  new URL(REL_ENV_PRODUCTION, import.meta.url),
);
/** `.vampus.yml`, el fichero que gobierna el versionado del proyecto. */
const VAMPUS_PATH = fileURLToPath(new URL(REL_VAMPUS, import.meta.url));

const ENV_PRODUCTION_FILE = "frontend/.env.production";

function readTextIfExists(path: string): string | null {
  return existsSync(path) ? readFileSync(path, "utf8") : null;
}

/**
 * Lee `frontend/.env.production`. Si no existe, deja constancia con un mensaje
 * que lo dice en lugar de reventar en la lectura.
 */
function readEnvProduction(): string | null {
  const env = readTextIfExists(ENV_PRODUCTION_PATH);
  expect(
    env,
    `debe existir ${ENV_PRODUCTION_PATH}: el build de producción tiene que declarar la versión publicada`,
  ).not.toBeNull();
  return env;
}

/** `current_version` de `.vampus.yml`, o `null` si no se declara. */
function currentVersion(): string | null {
  const vampus = readTextIfExists(VAMPUS_PATH);
  if (vampus === null) {
    return null;
  }
  return vampus.match(/^current_version:[ \t]*(\S+)[ \t]*$/m)?.[1] ?? null;
}

/**
 * El bloque de `replaces` de `.vampus.yml` que sustituye en el fichero dado, o
 * `null` si no hay ninguno. El YAML se parte por cada `- file:` en vez de
 * parsearlo: no se añaden dependencias por un fichero de tres claves.
 */
function vampusReplacement(file: string): string | null {
  const vampus = readTextIfExists(VAMPUS_PATH);
  if (vampus === null) {
    return null;
  }
  return (
    vampus
      .split(/^[ \t]*-[ \t]+file:[ \t]*/m)
      .slice(1)
      .find((block) => block.startsWith(file)) ?? null
  );
}

describe("build de producción — la versión publicada", () => {
  it("declara VITE_VERSION con una versión semántica en frontend/.env.production", () => {
    const env = readEnvProduction();
    if (env === null) {
      return;
    }

    const declared = env.match(/^VITE_VERSION=(\d+\.\d+\.\d+)[ \t]*$/m)?.[1];
    expect(
      declared,
      `${ENV_PRODUCTION_FILE} debe declarar VITE_VERSION con una versión semántica (X.Y.Z)`,
    ).toBeTruthy();
  });

  it("declara la misma versión que gobierna el versionado del proyecto", () => {
    const env = readEnvProduction();
    if (env === null) {
      return;
    }

    const declared = env.match(/^VITE_VERSION=(\S+)[ \t]*$/m)?.[1] ?? null;
    const governed = currentVersion();
    expect(
      governed,
      "`.vampus.yml` debe declarar `current_version`, que es la versión que gobierna el proyecto",
    ).not.toBeNull();

    expect(declared).toBe(governed);
  });

  it("mantiene frontend/.env.production en la lista de sustituciones del versionado", () => {
    const block = vampusReplacement(ENV_PRODUCTION_FILE);
    expect(
      block,
      `\`.vampus.yml\` debe mantener \`${ENV_PRODUCTION_FILE}\` en \`replaces\`, para que el versionado escriba VITE_VERSION`,
    ).not.toBeNull();
    if (block === null) {
      return;
    }

    const pattern = block.match(/^[ \t]*pattern:[ \t]*(.*)$/m)?.[1] ?? "";
    expect(
      pattern,
      `la sustitución de \`${ENV_PRODUCTION_FILE}\` debe llevar un \`pattern\` que mencione VITE_VERSION`,
    ).toContain("VITE_VERSION");
    expect(
      pattern,
      `la sustitución de \`${ENV_PRODUCTION_FILE}\` debe escribir \`{{current_version}}\``,
    ).toContain("{{current_version}}");
  });
});
