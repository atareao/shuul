# Spec Delta

## Purpose

Change the Top Rules chart to display rule names instead of rule IDs. The `read_top_rules` handler now looks up rule names from the in-memory rules cache.

## ADDED Requirements

### Requirement: Top Rules returns rule names

The `read_top_rules` handler returns `Vec<(String, i32, f32)>` where the first element is the rule **name** (was rule ID as string). For each rule ID from `StatsCollector::get_top_rules()`, look up the corresponding `CacheRule` in `AppState.rules`. If found, use `rule.name`; if not found, fall back to `"Unknown rule #{id}"`.

#### Scenario: Happy path — all rules exist in cache
Given the stats collector has recorded hits for rules with IDs 1, 2, 3
And the rules cache contains rules with names "Auth Guard", "Path Scanner", "Geo Block"
When the user navigates to the Charts page
Then the Top Rules pie chart shows "Auth Guard", "Path Scanner", "Geo Block" as labels

#### Scenario: Rule deleted from DB but still in stats
Given the stats collector has recorded hits for rule ID 99
And no rule with ID 99 exists in the rules cache
When the user navigates to the Charts page
Then the Top Rules pie chart shows "Unknown rule #99" as the label

#### Scenario: Empty top rules
Given the stats collector has no recorded rule hits
When the user navigates to the Charts page
Then the Top Rules pie chart shows "No rule data available"