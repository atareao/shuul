-- Índices para la tabla requests (consultas más frecuentes)
CREATE INDEX IF NOT EXISTS idx_requests_created_at ON requests (created_at);
CREATE INDEX IF NOT EXISTS idx_requests_rule_id ON requests (rule_id);
CREATE INDEX IF NOT EXISTS idx_requests_country_code ON requests (country_code);
CREATE INDEX IF NOT EXISTS idx_requests_country_name ON requests (country_name);

-- Índice compuesto para la tabla rules (carga en caché al inicio)
CREATE INDEX IF NOT EXISTS idx_rules_active_weight ON rules (active, weight ASC);
