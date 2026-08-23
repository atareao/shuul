-- Eliminar las reglas dummy creadas por la migración 20251015000000_seed_rules.

DELETE FROM rules WHERE id BETWEEN 1 AND 10;
