# Tasks: template-filters-overflow

## 1. RED (tests que fallan primero)

- [x] 1.1 Frontend: test que fija que el tag del `path` de una plantilla con un valor de 314 caracteres —el real de `Scanner directorios+configs+paneles`— lleva el estilo de truncado (`max-width: 100%`, `overflow: hidden`, `text-overflow: ellipsis` y `white-space: nowrap`) al abrir el diálogo de aplicar plantilla.
- [x] 1.2 Frontend: test que fija que al pasar el ratón por ese tag aparece un tooltip con el regex **completo**.
- [x] 1.3 Frontend: test que fija que el texto completo sigue en el DOM del tag (el truncado es visual, no recorta el valor).
- [x] 1.4 Frontend: test que fija que los tags del detalle expandible de una fila llevan el mismo estilo y su propio tooltip.
- [x] 1.5 Frontend: test que fija que los tags del scope del diálogo llevan el mismo estilo y su propio tooltip.
- [x] 1.6 Frontend: test que fija que un filtro corto sigue mostrándose completo en una línea, sin recortar el texto.
- [x] 1.7 Ejecutar `npx vitest run`: los nuevos fallan y **todos** los existentes siguen en verde.

## 2. GREEN (implementación mínima)

- [x] 2.1 `templates_page.tsx`: la constante de estilo de truncado de los tags que muestran valores.
- [x] 2.2 `templates_page.tsx`: aplicar el estilo a los tags de filtros y de scope de la previsualización del diálogo, y envolver cada uno en un `Tooltip` con el valor completo.
- [x] 2.3 `templates_page.tsx`: lo mismo en los tags de `renderFilterDetails`.
- [x] 2.4 `npx vitest run` en verde y `tsc -b --force` limpio.

## 3. REFACTOR

- [x] 3.1 `npm run lint` y `tsc -b --force` limpios, sin `eslint-disable` ni `any`.
- [x] 3.2 Un único punto de construcción del tag con su estilo y su `Tooltip`, para no repetir la misma construcción veinte veces en el diálogo.
- [x] 3.3 Comprobar que no queda ningún tag de valor sin `Tooltip` en `templates_page.tsx` (el del perfil de rate limit incluido, que también muestra un nombre de la plantilla).

## 4. Verificación manual en producción (la hace el usuario)

- [ ] 4.1 En el navegador: abrir *Apply* en `Scanner directorios+configs+paneles` → los filtros se quedan dentro del diálogo, sin barra horizontal, el path se ve recortado con puntos suspensivos y al pasar el ratón aparece entero.
- [ ] 4.2 En el navegador: expandir esa misma fila → el detalle tampoco desborda y su tooltip también trae el valor completo.
- [ ] 4.3 En el navegador: aplicar una plantilla de filtros cortos → todo se ve como antes, sin elipsis.
