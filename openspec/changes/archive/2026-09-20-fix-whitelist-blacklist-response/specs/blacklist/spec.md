# blacklist Specification (Delta)

## ADDED Requirements

### Requirement: Blacklist list endpoint returns PagedResponse
El endpoint `GET /api/v1/blacklist` ahora devuelve un `PagedResponse` envolviendo el array de entries, para compatibilidad con el frontend.

#### Scenario: List all blacklist entries returns PagedResponse
Given the blacklist has 3 entries
When an administrator calls GET `/api/v1/blacklist`
Then the response status is 200
And the response body contains `status: 200`
And the response body contains `data` with all 3 entries with their id, entry_type, value, description, and created_at
And the response body contains `pagination` with page=1, records=3, pages=1

#### Scenario: Empty blacklist returns PagedResponse with empty data
Given the blacklist is empty
When an administrator calls GET `/api/v1/blacklist`
Then the response status is 200
And the response body contains `data` as an empty array
And the response body contains `pagination` with records=0, pages=0