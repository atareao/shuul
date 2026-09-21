# rules/import-export Specification

## Purpose

Import and export rules as JSON files from the Rules page UI. Export downloads all rules as a JSON file. Import reads a JSON file and upserts rules by name.

## Requirements

### Requirement: Export rules as JSON

The Rules page SHALL provide an "Export" button in the header area. When clicked, it SHALL fetch `GET /api/v1/rules/export`, parse the JSON response, and trigger a browser download of a file named `shuul-rules-<YYYY-MM-DD>.json` containing the rules array.

#### Scenario: Happy path export
Given the user is on the Rules page
When the user clicks the Export button
Then the browser downloads a file named `shuul-rules-2026-09-21.json` containing all rules as a JSON array

#### Scenario: Export API error
Given the export API returns an error
When the user clicks the Export button
Then the user sees an error message via Ant Design `message.error`

### Requirement: Import rules from JSON

The Rules page SHALL provide an "Import" button in the header area. When clicked, it SHALL open a file picker accepting `.json` files. After the user selects a file, it SHALL read the file, parse it as a JSON array of rule objects, and POST it to `POST /api/v1/rules/import` with payload `{ "rules": [...] }`. On success, the table SHALL refresh automatically.

#### Scenario: Happy path import
Given the user is on the Rules page
When the user clicks Import, selects a valid JSON file with rules
Then the rules are imported via POST
And a success message is shown with the count of imported rules
And the rules table refreshes automatically

#### Scenario: Invalid JSON file
Given the user clicks Import and selects a file
When the file contains malformed JSON
Then an error message is shown
And no POST request is sent

#### Scenario: Empty array import
Given the user clicks Import and selects a file
When the file contains an empty JSON array `[]`
Then the import proceeds
And a success message shows "Imported 0 rules"

#### Scenario: Wrong format (not an array)
Given the user clicks Import and selects a file
When the file contains valid JSON but not an array
Then an error message "Invalid format: expected an array of rules" is shown
And no POST request is sent

#### Scenario: Import API error
Given the user clicks Import and selects a valid file
When the import API returns an error
Then the user sees an error message

### Requirement: Import button shows loading state

While the import request is in flight, the Import button SHALL show a loading spinner and be disabled to prevent double-submission.

#### Scenario: Loading during import
Given the user has selected a file for import
While the import request is in flight
Then the Import button shows a loading spinner and is disabled

#### Scenario: Button returns to normal after import
Given the import request completes (success or failure)
Then the Import button returns to its normal state

### Requirement: Import validates file type

The file picker SHALL only accept `.json` files via the `accept` attribute.

#### Scenario: JSON file accepted
Given the user clicks Import
When the file picker opens
Then it only accepts files with `.json` extension

#### Scenario: Non-JSON file rejected
Given the user clicks Import
When the file picker opens
Then files other than `.json` are filtered out by the browser