-- Crear reglas dummy necesarias para la migración de datos fake.
-- Esta migración debe ejecutarse antes de 20251121173011_requests-fake-data.

INSERT INTO rules (id, weight, allow, store, active) VALUES
    (1,  1,  true,  true,  true),
    (2,  2,  true,  true,  true),
    (3,  3,  true,  false, true),
    (4,  4,  false, true,  true),
    (5,  5,  true,  true,  false),
    (6,  6,  true,  false, false),
    (7,  7,  false, false, true),
    (8,  8,  true,  true,  true),
    (9,  9,  false, true,  false),
    (10, 10, true,  false, true);
