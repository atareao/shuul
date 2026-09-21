# Blacklist

## ADDED Requirements

### Requirement: Blacklist entries block requests before all other checks
Blacklist entries represent sources that should always be denied. When a request matches any blacklist entry, it is DENIED with 403 FORBIDDEN before evaluating banned IPs or WAF rules.

#### Scenario: Blacklist an IP address
Given an administrator creates a blacklist entry with `entry_type = "ip"` and `value = "10.0.0.99"`
When a request arrives from IP `10.0.0.99`
Then the request is DENIED with 403 FORBIDDEN

#### Scenario: Blacklist a CIDR range
Given an administrator creates a blacklist entry with `entry_type = "range"` and `value = "192.168.0.0/16"`
When a request arrives from IP `192.168.1.1`
Then the request is DENIED with 403 FORBIDDEN

#### Scenario: Blacklist a country
Given an administrator creates a blacklist entry with `entry_type = "country"` and `value = "RU"`
When a request arrives with country code `RU`
Then the request is DENIED with 403 FORBIDDEN

#### Scenario: Empty blacklist
Given the blacklist is empty
When any request arrives
Then the blacklist phase passes without matching and evaluation continues to the banned IP phase

### Requirement: Blacklist CRUD API
Administrators can manage blacklist entries via JWT-protected API endpoints.

#### Scenario: List all blacklist entries
Given the blacklist has 3 entries
When an administrator calls GET `/api/v1/blacklist`
Then the response contains all 3 entries with their id, entry_type, value, description, and created_at

#### Scenario: Delete a blacklist entry
Given a blacklist entry with id=1 exists
When an administrator calls DELETE `/api/v1/blacklist` with id=1
Then the entry is removed and subsequent requests from that IP are no longer blacklisted

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