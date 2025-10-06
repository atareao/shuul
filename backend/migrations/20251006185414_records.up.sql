CREATE TABLE IF NOT EXISTS records (
    id SERIAL PRIMARY KEY,
    ip_address VARCHAR,
    protocol VARCHAR,
    fqdn VARCHAR,
    path VARCHAR,
    city_name VARCHAR,
    country_name VARCHAR,
    country_code VARCHAR,
    rule_id INTEGER,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP
);
