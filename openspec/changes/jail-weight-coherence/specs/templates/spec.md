# templates Specification (delta)

## MODIFIED Requirements

### Requirement: Weight alignment with pipeline

Los pesos (weight) de los templates SHALL estar alineados con el flujo weight ASC del pipeline WAF (primera regla que matchea gana). Los templates del pipeline JAIL SHALL declarar peso 0: el pipeline de Jail itera todas las reglas que casan, sin `break`, así que su orden no decide nada y un peso distinto de 0 sería una decisión sin efecto que la interfaz, además, oculta.

#### Scenario: WAF must-have have lowest weights
- **WHEN** se examinan los templates WAF must-have
- **THEN** sus pesos SHALL estar en rango 10-100 (evalúan primero)

#### Scenario: WAF specific templates have medium weights
- **WHEN** se examinan los templates WAF específicos
- **THEN** sus pesos SHALL estar en rango 100-300

#### Scenario: JAIL catch-all have lowest weights
- **WHEN** se examinan los templates JAIL catch-all/scanner
- **THEN** sus pesos SHALL ser 0 (el mínimo posible: el pipeline de Jail no ordena, así que no hay rango que respetar)

#### Scenario: JAIL service protection have medium weights
- **WHEN** se examinan los templates JAIL de protección de servicios
- **THEN** sus pesos SHALL ser 0, igual que los catch-all, porque el pipeline de Jail no ordena y el peso no decide
