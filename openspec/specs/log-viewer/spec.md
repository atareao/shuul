# log-viewer Specification

## Purpose
TBD - created by archiving change persist-log-viewer-state. Update Purpose after archive.

## Requirements

### Requirement: Persist filter and auto-refresh state across views

The Log Viewer page persists `hiddenEvents` (event type filter selection) and `autoRefresh` toggle in `localStorage` so the user's view configuration survives navigation to other admin pages and back.

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
And `autoRefresh` is `false`

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
