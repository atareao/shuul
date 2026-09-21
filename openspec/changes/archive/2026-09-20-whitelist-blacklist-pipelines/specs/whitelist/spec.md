# Whitelist

## ADDED Requirements

### Requirement: Whitelist entries bypass all security checks
Whitelist entries represent trusted sources that should never be blocked. When a request matches any whitelist entry, it is ALLOWED immediately without evaluating blacklist, banned IPs, or WAF rules.

#### Scenario: Whitelist an IP address
Given an administrator creates a whitelist entry with `entry_type = "ip"` and `value = "192.168.1.100"`
When a request arrives from IP `192.168.1.100`
Then the request is ALLOWED immediately (200 OK) without evaluating blacklist, banned IPs, or WAF rules

#### Scenario: Whitelist a CIDR range
Given an administrator creates a whitelist entry with `entry_type = "range"` and `value = "10.0.0.0/8"`
When a request arrives from IP `10.0.0.50`
Then the request is ALLOWED immediately (200 OK)

#### Scenario: Whitelist a country
Given an administrator creates a whitelist entry with `entry_type = "country"` and `value = "ES"`
When a request arrives with country code `ES`
Then the request is ALLOWED immediately (200 OK)

#### Scenario: Whitelisted IP is also banned
Given an IP `10.0.0.1` is both whitelisted and banned by JAIL
When a request arrives from that IP
Then the request is ALLOWED (whitelist takes precedence over bans)

#### Scenario: Empty whitelist
Given the whitelist is empty
When any request arrives
Then the whitelist phase passes without matching and evaluation continues to the blacklist phase

### Requirement: Whitelist CRUD API
Administrators can manage whitelist entries via JWT-protected API endpoints.

#### Scenario: List all whitelist entries
Given the whitelist has 3 entries
When an administrator calls GET `/api/v1/whitelist`
Then the response contains all 3 entries with their id, entry_type, value, description, and created_at

#### Scenario: Delete a whitelist entry
Given a whitelist entry with id=1 exists
When an administrator calls DELETE `/api/v1/whitelist` with id=1
Then the entry is removed and subsequent requests from that IP are no longer whitelisted

#### Scenario: Invalid entry_type returns 400
When an administrator creates a whitelist entry with `entry_type = "invalid"`
Then the API returns a 400 Bad Request error

#### Scenario: Invalid IP address value returns 400
When an administrator creates a whitelist entry with `entry_type = "ip"` and `value = "not-an-ip"`
Then the API returns a 400 Bad Request error

#### Scenario: Invalid CIDR range value returns 400
When an administrator creates a whitelist entry with `entry_type = "range"` and `value = "not-a-cidr"`
Then the API returns a 400 Bad Request error

#### Scenario: Invalid country code value returns 400
When an administrator creates a whitelist entry with `entry_type = "country"` and `value = "TOO_LONG"`
Then the API returns a 400 Bad Request error