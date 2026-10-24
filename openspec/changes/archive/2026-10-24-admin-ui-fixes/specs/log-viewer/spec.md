## MODIFIED Requirements

### Requirement: Persist filter and auto-refresh state across views

La página Log Viewer SHALL persistir `hiddenEvents` (selección de tipos de evento
ocultos) y el interruptor `autoRefresh` en `localStorage`, y `localStorage` SHALL ser
la **única** fuente de verdad de esa configuración. La página NO SHALL leer ni
escribir la query string de la URL para esta finalidad: una URL que llegue con
`?hidden=…&autoRefresh=…` NO SHALL alterar la configuración persistida ni el estado
de la página.

Cuando no haya nada persistido, `hiddenEvents` SHALL ser `[]` y `autoRefresh` SHALL
ser **`true`**: el visor arranca refrescándose solo.

#### Scenario: State is restored from localStorage on mount
Given the user previously selected events "banned" and "block" as hidden and enabled auto-refresh
When the user navigates to `/admin/logs`
Then `hiddenEvents` contains `["banned", "block"]`
And `autoRefresh` is `true`
And the polling timer is started

#### Scenario: State defaults when localStorage is empty
Given no persisted state exists in localStorage
When the user navigates to `/admin/logs`
Then `hiddenEvents` is `[]`
And `autoRefresh` is `true`
And the polling timer is started

#### Scenario: Toggling an event filter persists immediately
Given the user is on the Log Viewer page with `hiddenEvents = []`
When the user clicks the "banned" event button
Then `hiddenEvents` becomes `["banned"]`
And localStorage contains `hiddenEvents: ["banned"]`

#### Scenario: Toggling auto-refresh persists immediately
Given the user is on the Log Viewer page with `autoRefresh = false`
When the user toggles auto-refresh on
Then `autoRefresh` becomes `true`
And localStorage contains `autoRefresh: true`

#### Scenario: State survives navigation away and back
Given the user has `hiddenEvents = ["banned"]` and `autoRefresh = true`
When the user navigates to `/admin/rules` and then back to `/admin/logs`
Then `hiddenEvents` is still `["banned"]`
And `autoRefresh` is still `true`
And the polling timer is running

#### Scenario: The URL query string is neither read nor written
Given the user has `hiddenEvents = ["banned"]` and `autoRefresh = true`
When the user toggles an event filter
Then `window.location.search` is not modified
When the user then loads `/admin/logs?hidden=admitted&autoRefresh=false`
Then `hiddenEvents` is still `["banned"]`
And `autoRefresh` is still `true`
