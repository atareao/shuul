# Blacklist

## MODIFIED Requirements

### Requirement: Blacklist CRUD API
Administrators can manage blacklist entries via JWT-protected API endpoints.

#### Scenario: List all blacklist entries
Given the blacklist has 3 entries
When an administrator calls GET `/api/v1/blacklist`
Then the response contains all 3 entries with their id, entry_type, value, description, and created_at

#### Scenario: Delete a blacklist entry
Given a blacklist entry with id=1 exists
When an administrator calls DELETE `/api/v1/blacklist?id=1`
Then the response status is 200
And the entry is removed

#### Scenario: Invalid entry_type returns 400
When an administrator creates a blacklist entry with `entry_type = "invalid"`
Then the API returns a 400 Bad Request error

#### Scenario: Invalid IP address value returns 400
When an administrator creates a blacklist entry with `entry_type = "ip"` and `value = "not-an-ip"`
Then the API returns a 400 Bad Request error

#### Scenario: Invalid CIDR range value returns 400
When an administrator creates a blacklist entry with `entry_type = "range"` and `value = "not-a-cidr"`
Then the API returns a 400 Bad Request error

#### Scenario: Invalid country code value returns 400
When an administrator creates a blacklist entry with `entry_type = "country"` and `value = "TOO_LONG"`
Then the API returns a 400 Bad Request error

#### Scenario: Duplicate entry_type+value returns 409 on create
Given a blacklist entry exists with entry_type="ip" and value="10.0.0.99"
When an administrator creates a blacklist entry with entry_type="ip" and value="10.0.0.99"
Then the API returns 409 Conflict
And the error message indicates the entry already exists

#### Scenario: Duplicate entry_type+value returns 409 on update
Given a blacklist entry with id=1 exists with entry_type="ip" and value="10.0.0.99"
And a blacklist entry with id=2 exists with entry_type="ip" and value="192.168.1.1"
When an administrator updates entry id=2 to value="10.0.0.99"
Then the API returns 409 Conflict
And the error message indicates the entry already exists

#### Scenario: Same value with different entry_type is allowed
Given a blacklist entry exists with entry_type="ip" and value="10.0.0.99"
When an administrator creates a blacklist entry with entry_type="range" and value="10.0.0.99"
Then the API returns 201 Created
And the entry is successfully created