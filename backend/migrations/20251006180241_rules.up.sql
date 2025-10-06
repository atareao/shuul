CREATE TABLE IF NOT EXISTS rules (
    id SERIAL PRIMARY KEY,
    norder INT NOT NULL,
    allow BOOLEAN NOT NULL,
    ip_address VARCHAR,
    protocol VARCHAR,
    fqdn VARCHAR,
    path VARCHAR,
    city_name VARCHAR,
    country_name VARCHAR,
    country_code VARCHAR,
    active BOOLEAN DEFAULT TRUE,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP
);
