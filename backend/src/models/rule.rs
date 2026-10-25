//! # Modelo de reglas de filtrado
//!
//! Define [`Rule`], [`NewRule`], [`UpdateRule`] y [`CacheRule`].
//! Las reglas determinan si una petición HTTP debe ser permitida,
//! denegada y/o almacenada en la base de datos.
//!
//! [`CacheRule`] envuelve una [`Rule`] con su conjunto de condiciones
//! precompilado —cada una con su patrón y la pipeline de transformaciones— para
//! el matcheo rápido en memoria.

use crate::constants::DEFAULT_LIMIT;
use crate::constants::DEFAULT_PAGE;
use crate::models::NewRequest;
use crate::models::condition::{self, Collection, Condition, Operator};
use crate::models::error::AppError;
use crate::models::transformation::TransformationPipeline;
use chrono::{DateTime, Utc};
use regex::Regex;
use serde::{Deserialize, Serialize};
use sqlx::{Error, Row, SqliteConnection, SqlitePool, query, sqlite::SqliteRow};
use std::collections::BTreeMap;
use std::fmt::Write;
use tracing::{debug, error, warn};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Rule {
    pub id: i32,
    pub name: String,
    pub description: String,
    pub weight: i32,
    pub mode: String,
    pub pipeline: String,
    /// La especificación `t:` de transformaciones que se aplica al valor de la
    /// petición antes de compararlo con el patrón. La cadena vacía es la
    /// pipeline vacía: el valor no se toca.
    pub transformations: String,
    pub allow: bool,
    pub is_tor: Option<bool>,
    pub ip_address: Option<String>,
    pub protocol: Option<String>,
    pub fqdn: Option<String>,
    pub path: Option<String>,
    pub query: Option<String>,
    pub city_name: Option<String>,
    pub country_name: Option<String>,
    pub country_code: Option<String>,
    pub user_agent: Option<String>,
    pub method: Option<String>,
    pub referer: Option<String>,
    pub content_type: Option<String>,
    pub accept_language: Option<String>,
    pub x_request_id: Option<String>,
    pub rate_limit_profile_id: Option<i32>,
    pub rate_limit_profile_name: Option<String>,
    pub active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct RuleInfoCounts {
    pub total: i64,
    pub active: i64,
}

#[derive(Debug, Clone)]
pub struct CacheRule {
    pub rule: Rule,
    /// El conjunto ordenado de condiciones de la regla. En **producción**
    /// ([`CacheRule::read_all_active`]) se **lee** de la tabla `rule_conditions`
    /// ([`Rule::read_conditions`]), que es la fuente persistida del matcheo; la
    /// proyección desde los 14 campos de [`Rule`]
    /// ([`crate::models::condition::project`]) es la que usa la **escritura**
    /// ([`Rule::write_conditions`]) para poblar esa tabla, y el constructor en
    /// memoria [`CacheRule::from_rule`]. Un conjunto vacío es una regla neutral;
    /// [`CacheRule::evaluate`] lo recorre en AND con cortocircuito.
    pub conditions: Vec<Condition>,
}

/// Resultado de evaluar una regla contra una petición.
///
/// La evidencia es el mapa `campo restringido → valor transformado que se
/// comparó`. Se obtiene del MISMO recorrido que decide `matched`
/// ([`CacheRule::evaluate`]).
#[derive(Debug, Clone, Default)]
pub struct MatchOutcome {
    /// Si la regla casa la petición.
    pub matched: bool,
    /// La evidencia del matcheo: por cada campo que la regla restringe y que
    /// casó, el valor transformado que se comparó. Es `None` cuando la regla
    /// **no** casa, cuando la pipeline es vacía o cuando la regla no restringe
    /// ninguno de los 14 campos.
    pub evidence: Option<BTreeMap<String, String>>,
}

impl CacheRule {
    /// Construye un `CacheRule` con los regex precompilados desde una [`Rule`] y
    /// una pipeline de transformaciones **ya resuelta**.
    ///
    /// Es el constructor común de [`CacheRule::from_rule`] y
    /// [`CacheRule::read_all_active`]; existe para que la especificación se
    /// parsee **una sola vez** por fila.
    fn from_rule_and_pipeline(rule: &Rule, transformations: &TransformationPipeline) -> Self {
        Self {
            rule: rule.clone(),
            conditions: condition::project(rule, transformations),
        }
    }

    /// Construye un `CacheRule` con los regex precompilados desde una [`Rule`].
    ///
    /// La especificación de transformaciones se resuelve con
    /// [`TransformationPipeline::parse`], y una que no se pueda leer cae a la
    /// pipeline vacía ([`TransformationPipeline::default`]), de modo que esta
    /// función es **total**. Es el camino de las reglas construidas a mano —los
    /// tests y la conversión [`From<Rule>`]—; el camino de **producción** es
    /// [`CacheRule::read_all_active`], que es donde la fila con una
    /// especificación irresoluble se **excluye** de la caché en vez de degradarla
    /// en silencio a la pipeline vacía.
    #[must_use]
    pub fn from_rule(rule: &Rule) -> Self {
        Self::from_rule_and_pipeline(
            rule,
            &TransformationPipeline::parse(&rule.transformations).unwrap_or_default(),
        )
    }

    /// Lee todas las reglas activas desde la base de datos.
    ///
    /// Cada fila pasa por [`TransformationPipeline::parse`]: una especificación
    /// que no se pueda leer **excluye** la regla de la caché —no la degrada— y
    /// emite un `error!` que nombra la regla y el token. Cargarla con las
    /// transformaciones que sí se reconocen, o con ninguna, la haría decidir algo
    /// distinto de lo que su autor escribió, que es el fallo silencioso que se
    /// evita.
    ///
    /// El conjunto de condiciones de cada regla se construye **leyendo
    /// `rule_conditions`** ordenado por `position` ([`Rule::read_conditions`]), no
    /// proyectando los 14 campos en memoria: la tabla es la fuente persistida del
    /// matcheo. La proyección desde esas filas es equivalente a la de los 14
    /// campos y la columna `is_tor` —la escritura usa la misma proyección—, de
    /// modo que la decisión y la evidencia no cambian. La colección `is_tor` es
    /// una condición más del conjunto, evaluada en AND con las demás.
    ///
    /// Una fila irresoluble **no** hace que la función devuelva `Err`: se excluye
    /// y el resto de reglas se carga con normalidad.
    ///
    /// # Errors
    ///
    /// Returns an error if the database query fails.
    pub async fn read_all_active(pool: &SqlitePool) -> Result<Vec<Self>, Error> {
        let sql = "SELECT * FROM rules WHERE active = 1 ORDER BY weight ASC";
        let rows = query(sql).fetch_all(pool).await?;

        let mut rules = Vec::with_capacity(rows.len());
        for row in &rows {
            let rule = Rule::from_row(row);
            match TransformationPipeline::parse(&rule.transformations) {
                Ok(_) => {
                    let conditions = Rule::read_conditions(pool, rule.id).await?;
                    rules.push(Self { rule, conditions });
                },
                Err(parse_error) => {
                    error!(
                        rule_id = rule.id,
                        rule_name = %rule.name,
                        token = parse_error.token(),
                        "regla excluida de la caché: su especificación de \
                         transformaciones no se puede leer: {parse_error}"
                    );
                },
            }
        }

        Ok(rules)
    }

    /// Evalúa la regla contra una petición y devuelve el resultado **y** su
    /// evidencia.
    ///
    /// Es el **único** recorrido del matcheo: recorre el conjunto ordenado de
    /// condiciones —cada una por [`CacheRule::condition_matches`]— y de ese mismo
    /// recorrido salen tanto `matched` como la evidencia. NO existe un segundo
    /// recorrido que reevalúe las condiciones:
    /// dos recorridos divergirían y la evidencia dejaría de describir la decisión
    /// que de verdad se tomó (lo verifica la tarea 5.3 del change).
    ///
    /// El conjunto se evalúa en AND con **cortocircuito**: la primera condición
    /// que no casa detiene el recorrido.
    ///
    /// La evidencia es `Some` **solo** cuando la regla **casa**, la pipeline es no
    /// vacía **y** la regla restringe al menos uno de los 14 campos: con la
    /// pipeline vacía nadie inserta nada en el mapa, y con la pipeline no vacía pero
    /// sin ninguna condición tampoco. Y una regla que **no** casa devuelve `None`
    /// aunque alguna condición evaluada antes de la primera que falló haya casado:
    /// el cortocircuito hace que la evidencia describa la decisión que de verdad se
    /// tomó.
    #[must_use]
    pub fn evaluate(&self, request: &NewRequest) -> MatchOutcome {
        // La evidencia se acumula en el MISMO recorrido que decide `matched`.
        let mut evidence = BTreeMap::new();
        // Si CUALQUIERA de las condiciones devuelve 'false', el metodo devuelve
        // 'false' y se detiene: la cadena `&&` y este recorrido cortocircuitan igual.
        let mut fields_matched = true;
        for condition in &self.conditions {
            if !self.condition_matches(condition, request, &mut evidence) {
                fields_matched = false;
                break;
            }
        }
        let matched = fields_matched;

        MatchOutcome {
            matched,
            // La clave solo existe cuando hay algo que decir: un mapa vacío se
            // colapsa a `None` para que el evento no lleve un objeto vacío. Y una
            // regla que NO casa no emite evidencia aunque alguna condición evaluada
            // antes de la primera que falló haya casado: el cortocircuito hace que
            // la evidencia describa la decisión que de verdad se tomó.
            evidence: if !matched || evidence.is_empty() {
                None
            } else {
                Some(evidence)
            },
        }
    }

    /// Indica si la regla casa la petición.
    ///
    /// Es un envoltorio de [`CacheRule::evaluate`]: su resultado es exactamente
    /// `self.evaluate(request).matched`.
    #[must_use]
    pub fn matches(&self, request: &NewRequest) -> bool {
        self.evaluate(request).matched
    }

    /// Evalúa **una** condición contra la petición.
    ///
    /// Es el **único** punto de decisión del matcheo: [`CacheRule::evaluate`] lo
    /// invoca una vez por cada condición del conjunto. El contrato es:
    ///
    /// - la colección **está** en la petición → la pipeline de la condición se
    ///   aplica **al valor de la petición** —nunca al patrón, que se usa tal como
    ///   se compiló— y el operador decide sobre el resultado. Si casa y la
    ///   pipeline no está vacía, el valor **transformado** que se comparó queda en
    ///   `evidence` bajo [`crate::models::condition::Collection::field_name`];
    /// - la colección **no** está en la petición → la condición NO se cumple
    ///   (`false`) y se emite un `debug!` que nombra el campo y la regla, de modo
    ///   que el descarte no sea mudo. La pipeline no se aplica a un valor ausente.
    ///
    /// La colección booleana `is_tor` es la excepción: su valor no viene de
    /// `request_value` sino de [`NewRequest::is_tor`]. Decide el operador:
    /// `@truthy` cumple con `Some(true)`; `Some(false)` y `None` no cumplen y
    /// emiten el `debug!`; cualquier otro operador sobre `is_tor` es incoherente y
    /// no casa. La pipeline es vacía y la colección **no** anota evidencia.
    ///
    /// Las colecciones que la regla **no** restringe no generan condición: su
    /// ausencia del conjunto es el brazo neutral. Con la pipeline vacía
    /// [`TransformationPipeline::apply`] devuelve
    /// [`std::borrow::Cow::Borrowed`] y `evidence` no se toca: el matcheo queda
    /// byte a byte como estaba.
    fn condition_matches(
        &self,
        condition: &Condition,
        request: &NewRequest,
        evidence: &mut BTreeMap<String, String>,
    ) -> bool {
        // La colección booleana `is_tor` no se resuelve por
        // [`Collection::request_value`], que devuelve texto, sino desde el
        // booleano de la petición. **Decide el operador**: `@truthy` cumple con
        // `Some(true)`; `Some(false)` y la ausencia (`None`) NO cumplen y emiten
        // el `debug!` que nombra la colección y la regla —la ausencia es el brazo
        // fail-open, hermano del de los campos ausentes—. Cualquier **otro**
        // operador sobre `is_tor` es incoherente con la colección —la autoría lo
        // rechaza y la lectura lo descarta— y no casa, con diagnóstico, en vez de
        // casar por el mero hecho de ser `IsTor`. La pipeline es vacía por
        // contrato: un booleano no se transforma. La colección NO anota evidencia
        // (decisión explícita, revisable): la forma del evento de auditoría no
        // cambia.
        if condition.collection == Collection::IsTor {
            match &condition.operator {
                Operator::Truthy => {
                    if request.is_tor == Some(true) {
                        return true;
                    }
                    debug!(
                        field = condition.collection.field_name(),
                        key = ?condition.key,
                        rule_id = self.rule.id,
                        rule_name = %self.rule.name,
                        "condición no cumplida: la petición no es Tor (o no se sabe)"
                    );
                    return false;
                },
                Operator::Rx(_) => {
                    debug!(
                        field = condition.collection.field_name(),
                        operator = condition.operator.name(),
                        key = ?condition.key,
                        rule_id = self.rule.id,
                        rule_name = %self.rule.name,
                        "condición no cumplida: operador incoherente con la colección booleana"
                    );
                    return false;
                },
            }
        }
        let Some(value) = condition
            .collection
            .request_value(request, condition.key.as_deref())
        else {
            debug!(
                field = condition.collection.field_name(),
                key = ?condition.key,
                rule_id = self.rule.id,
                rule_name = %self.rule.name,
                "condición no cumplida: la petición no trae el valor"
            );
            return false;
        };
        let transformed = condition.pipeline.apply(value);
        let matched = match &condition.operator {
            Operator::Rx(regex) => regex.is_match(&transformed),
            // `@truthy` solo es válido para `is_tor`, que se resolvió arriba: una
            // combinación incoherente (operador booleano sobre una colección de
            // texto) no casa. La lectura ya descarta esas filas.
            Operator::Truthy => false,
        };
        if matched && !condition.pipeline.is_empty() {
            // La evidencia usa la clave compuesta `campo[clave]` para las
            // colecciones con clave y el nombre del campo en cualquier otro caso.
            let key = match (
                condition.collection.requires_key(),
                condition.key.as_deref(),
            ) {
                (true, Some(k)) => {
                    format!("{}[{}]", condition.collection.field_name(), k)
                },
                _ => condition.collection.field_name().to_string(),
            };
            evidence.insert(key, transformed.into_owned());
        }
        matched
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct NewRule {
    pub name: String,
    pub description: Option<String>,
    pub weight: Option<i32>,
    pub mode: Option<String>,
    pub pipeline: Option<String>,
    /// La especificación `t:` de transformaciones de la regla. Ausente o vacía
    /// es la pipeline vacía.
    pub transformations: Option<String>,
    pub allow: Option<bool>,
    pub is_tor: Option<bool>,
    pub ip_address: Option<String>,
    pub protocol: Option<String>,
    pub fqdn: Option<String>,
    pub path: Option<String>,
    pub query: Option<String>,
    pub city_name: Option<String>,
    pub country_name: Option<String>,
    pub country_code: Option<String>,
    pub user_agent: Option<String>,
    pub method: Option<String>,
    pub referer: Option<String>,
    pub content_type: Option<String>,
    pub accept_language: Option<String>,
    pub x_request_id: Option<String>,
    pub rate_limit_profile_id: Option<i32>,
    pub active: Option<bool>,
    /// Lista ordenada de condiciones de la autoría avanzada. Ausente o nula
    /// significa **modo simple**: el conjunto se proyecta desde los 14 campos.
    /// Presente significa **modo avanzado**: la lista es el conjunto persistido.
    /// Este campo se valida y se persiste en la tabla `rule_conditions` cuando
    /// está presente.
    #[serde(default)]
    pub conditions: Option<Vec<ConditionInput>>,
}

/// Representación de entrada de una condición de la autoría avanzada.
///
/// Es la forma serializable/deserializable de una condición: la colección por
/// nombre, la clave opcional, la pipeline, el operador y el patrón. El validador
/// la convierte en una [`Condition`] compilada y se persiste en la tabla
/// `rule_conditions` cuando está presente.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ConditionInput {
    pub collection: String,
    #[serde(default)]
    pub key: Option<String>,
    #[serde(default)]
    pub transformations: String,
    #[serde(default)]
    pub operator: Option<String>,
    pub pattern: String,
}

impl From<&Condition> for ConditionInput {
    /// Proyecta una condición compilada a su forma serializable.
    ///
    /// La representación es la misma que la de entrada —`collection`, `key`,
    /// `transformations`, `operator` y `pattern`—, de modo que el ciclo
    /// lectura→edición es estable: lo que se lee se puede reenviar tal cual.
    /// La pipeline viaja en su especificación canónica
    /// ([`TransformationPipeline::spec`]) y el operador en su nombre canónico
    /// ([`Operator::name`]): `@rx` con el patrón, o `@truthy` con patrón vacío
    /// para `is_tor`.
    fn from(condition: &Condition) -> Self {
        let pattern = match &condition.operator {
            Operator::Rx(regex) => regex.as_str(),
            Operator::Truthy => "",
        };
        Self {
            collection: condition.collection.field_name().to_string(),
            key: condition.key.clone(),
            transformations: condition.pipeline.spec().to_string(),
            operator: Some(condition.operator.name().to_string()),
            pattern: pattern.to_string(),
        }
    }
}

impl ConditionInput {
    /// Compila y valida una condición de la autoría avanzada.
    ///
    /// Convierte la forma serializable ([`ConditionInput`]) en la IR compilada
    /// ([`Condition`]) que el matcheo consume. Es el **único punto de
    /// validación** de una condición avanzada: lo invocan las tres vías de
    /// escritura —crear, editar e importar—, de modo que lo almacenado es
    /// siempre una condición bien formada.
    ///
    /// Reglas de validación:
    ///
    /// - la colección debe ser conocida ([`Collection::from_field_name`]);
    /// - la clave es obligatoria exactamente cuando la colección la exige
    ///   ([`Collection::requires_key`]) y no debe venir en caso contrario;
    /// - el operador es ausente o `@rx` para las colecciones de texto, y
    ///   `@truthy` **solo** para `is_tor` —con patrón vacío, sin clave y sin
    ///   transformaciones—; `@truthy` fuera de `is_tor` y `@rx` sobre `is_tor`
    ///   se rechazan;
    /// - el patrón debe compilar con [`Regex::new`] (para `is_tor`/`@truthy` es
    ///   vacío);
    /// - las transformaciones deben resolverse con
    ///   [`TransformationPipeline::parse`];
    /// - para `request_headers` la clave se normaliza a minúsculas, igual que
    ///   la extracción guarda los nombres de cabecera.
    ///
    /// # Errors
    ///
    /// Devuelve [`AppError::InvalidInput`] —que la API traduce a
    /// `400 Bad Request`— si la condición incumple cualquiera de las reglas
    /// anteriores.
    pub fn compile(&self) -> Result<Condition, AppError> {
        let Some(collection) = Collection::from_field_name(&self.collection) else {
            return Err(AppError::InvalidInput(format!(
                "colección desconocida: «{}»",
                self.collection
            )));
        };

        let key = if collection.requires_key() {
            match self.key.as_deref() {
                Some(key) if !key.is_empty() => Some(if collection == Collection::RequestHeaders {
                    key.to_ascii_lowercase()
                } else {
                    key.to_string()
                }),
                _ => {
                    return Err(AppError::InvalidInput(format!(
                        "la colección «{}» exige una clave no vacía",
                        collection.field_name()
                    )));
                },
            }
        } else {
            if self.key.is_some() {
                return Err(AppError::InvalidInput(format!(
                    "la colección «{}» no admite clave",
                    collection.field_name()
                )));
            }
            None
        };

        // El operador decide la forma de la condición. `@truthy` es el operador
        // booleano y **solo** es válido para la colección `is_tor`, que exige
        // patrón vacío, `key` ausente (ya validado arriba) y transformaciones
        // vacías. Las colecciones de texto admiten ausente o `@rx`. Cualquier otro
        // caso es `400`.
        let operator = match (collection, self.operator.as_deref()) {
            (Collection::IsTor, None | Some("@truthy")) => {
                if !self.pattern.is_empty() {
                    return Err(AppError::InvalidInput(
                        "la condición «is_tor» con «@truthy» no admite patrón".to_string(),
                    ));
                }
                if !self.transformations.is_empty() {
                    return Err(AppError::InvalidInput(
                        "la condición «is_tor» con «@truthy» no admite transformaciones"
                            .to_string(),
                    ));
                }
                Operator::Truthy
            },
            (Collection::IsTor, Some(other)) => {
                return Err(AppError::InvalidInput(format!(
                    "la colección «is_tor» exige el operador «@truthy», no «{other}»"
                )));
            },
            (_, Some("@truthy")) => {
                return Err(AppError::InvalidInput(format!(
                    "«@truthy» solo es válido para la colección «is_tor», no para «{}»",
                    collection.field_name()
                )));
            },
            (_, None | Some("@rx")) => {
                let regex = Regex::new(&self.pattern).map_err(|error| {
                    AppError::InvalidInput(format!(
                        "el patrón de la condición «{}» no compila: {error}",
                        collection.field_name()
                    ))
                })?;
                Operator::Rx(regex)
            },
            (_, Some(other)) => {
                return Err(AppError::InvalidInput(format!(
                    "operador no admitido: «{other}»"
                )));
            },
        };

        let pipeline = TransformationPipeline::parse(&self.transformations)
            .map_err(|error| AppError::InvalidInput(error.to_string()))?;

        Ok(Condition {
            collection,
            key,
            pipeline,
            operator,
        })
    }
}

/// Compila la lista opcional de condiciones de la autoría avanzada.
///
/// Devuelve `None` cuando la lista está ausente o es nula —modo simple: la
/// proyección se hará desde los 14 campos al escribir— y la lista compilada
/// cuando viene. Un fallo en cualquier condición aborta la compilación entera:
/// la regla no se almacena a medias.
///
/// # Errors
///
/// Propaga el [`AppError::InvalidInput`] de la primera condición que no compile.
pub fn compile_conditions(
    conditions: Option<&[ConditionInput]>,
) -> Result<Option<Vec<Condition>>, AppError> {
    conditions
        .map(|list| list.iter().map(ConditionInput::compile).collect())
        .transpose()
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct UpdateRule {
    pub id: i32,
    pub name: String,
    pub description: String,
    pub weight: i32,
    pub mode: String,
    pub pipeline: String,
    /// La especificación `t:` de transformaciones de la regla. Ausente es la
    /// pipeline vacía: el `#[serde(default)]` mantiene compatibles los `PATCH`
    /// que no la envían (el valor por defecto de la columna es la cadena vacía).
    #[serde(default)]
    pub transformations: String,
    pub allow: bool,
    pub is_tor: Option<bool>,
    pub ip_address: Option<String>,
    pub protocol: Option<String>,
    pub fqdn: Option<String>,
    pub path: Option<String>,
    pub query: Option<String>,
    pub city_name: Option<String>,
    pub country_name: Option<String>,
    pub country_code: Option<String>,
    pub user_agent: Option<String>,
    pub method: Option<String>,
    pub referer: Option<String>,
    pub content_type: Option<String>,
    pub accept_language: Option<String>,
    pub x_request_id: Option<String>,
    pub rate_limit_profile_id: Option<i32>,
    pub active: bool,
    /// Lista ordenada de condiciones de la autoría avanzada. Mismo contrato que
    /// en [`NewRule`]: ausente o nula = modo simple; presente = modo avanzado.
    /// Este campo se valida y se persiste en la tabla `rule_conditions` cuando
    /// está presente.
    #[serde(default)]
    pub conditions: Option<Vec<ConditionInput>>,
}

/// Nombre del pipeline del WAF: `first match wins` por `weight ASC`, que es el
/// único sitio donde el peso decide.
pub const WAF_PIPELINE: &str = "waf";

/// Nombre del pipeline cuyas reglas no se ordenan: itera **todas** las que casan.
pub const JAIL_PIPELINE: &str = "jail";

/// Comprueba que el pipeline sea uno de los admitidos: `waf` o `jail`.
///
/// La comparación es **exacta y sensible a mayúsculas**, igual que la del resto
/// del sistema (`shuul.rs` salta las reglas `jail`, `report.rs` salta las `waf`):
/// aceptar `JAIL` como sinónimo de `jail` sería admitir un valor que ninguna de
/// esas dos comparaciones reconoce, de modo que la regla participaría de un
/// pipeline —o de ninguno— sin que se le apliquen sus invariantes, señaladamente
/// la del peso 0.
///
/// Es el **único punto de validación** del pipeline; lo invocan las tres vías de
/// escritura del cliente —creación ([`Rule::create`]), edición ([`Rule::update`])
/// e importación ([`crate::http::rule::import_handler`])—.
///
/// # Errors
///
/// Devuelve [`AppError::InvalidInput`] —que la API traduce a `400 Bad Request`—
/// si el pipeline no es exactamente `waf` o `jail`.
pub fn validate_pipeline(pipeline: &str) -> Result<(), AppError> {
    if pipeline == WAF_PIPELINE || pipeline == JAIL_PIPELINE {
        return Ok(());
    }
    Err(AppError::InvalidInput(format!(
        "pipeline no admitido: «{pipeline}»; se admiten «{WAF_PIPELINE}» y «{JAIL_PIPELINE}»"
    )))
}

/// La especificación canónica con la que se almacena una regla.
///
/// Es el **único punto de validación y de canonización** de la especificación de
/// transformaciones; lo invocan las tres vías de escritura del cliente —creación
/// ([`Rule::create`]), edición ([`Rule::update`]) e importación
/// ([`crate::http::rule::import_handler`])—, de modo que lo almacenado es siempre
/// una especificación canónica: el marcador `none` deja de aparecer y la regla
/// declara la pipeline que de verdad se aplica.
///
/// # Errors
///
/// Devuelve [`AppError::InvalidInput`] —que la API traduce a `400 Bad Request`—
/// si la especificación contiene un token que no se reconoce. El mensaje nombra
/// el token y lista los válidos.
pub fn normalized_transformations(spec: &str) -> Result<String, AppError> {
    TransformationPipeline::parse(spec)
        .map(|pipeline| pipeline.spec().to_string())
        .map_err(|error| AppError::InvalidInput(error.to_string()))
}

/// Peso con el que se almacena una regla, en función de su pipeline.
///
/// El pipeline de Jail evalúa **todas** las reglas que casan, sin `break`: su orden
/// —y por tanto su peso— no decide nada, así que una regla de Jail se almacena
/// **siempre** con peso 0. El del WAF es `first match wins` por `weight ASC`, que es
/// el único sitio donde el número decide, y ahí se conserva el peso pedido.
///
/// Es el **único punto de coerción** del peso; lo invocan las tres vías de escritura
/// del cliente —creación ([`Rule::create`]), edición ([`Rule::update`]) e importación
/// ([`crate::http::rule::import_handler`])—, de modo que la API nunca confía en el
/// peso que trae la petición.
#[must_use]
pub fn normalized_weight(pipeline: &str, weight: i32) -> i32 {
    if pipeline == JAIL_PIPELINE { 0 } else { weight }
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct ReadRuleParams {
    pub id: Option<i32>,
    pub name: Option<String>,
    pub description: Option<String>,
    pub mode: Option<String>,
    pub pipeline: Option<String>,
    pub weight: Option<i32>,
    pub allow: Option<bool>,
    pub ip_address: Option<String>,
    pub protocol: Option<String>,
    pub fqdn: Option<String>,
    pub path: Option<String>,
    pub query: Option<String>,
    pub city_name: Option<String>,
    pub country_name: Option<String>,
    pub country_code: Option<String>,
    pub rate_limit_profile_name: Option<String>,
    pub active: Option<bool>,
    pub page: Option<u32>,
    pub limit: Option<u32>,
    pub sort_by: Option<String>,
    pub asc: Option<bool>,
}

impl From<Rule> for CacheRule {
    fn from(val: Rule) -> Self {
        Self::from_rule(&val)
    }
}

impl Rule {
    /// Reemplaza el conjunto persistido de condiciones de `rule`.
    ///
    /// Si `conditions` es `Some(lista)`, persiste **esa** lista —el conjunto de la
    /// autoría avanzada, en su orden—; si es `None`, proyecta los 14 campos de la
    /// regla con [`condition::project`] —el modo simple—. En ambos casos **borra**
    /// las filas previas de la regla antes de insertar una fila por condición, con
    /// `position` = índice en el vector (contigua 0-based). El reemplazo, no el
    /// apéndice, es lo que hace que editar una regla deje exactamente el conjunto
    /// de la nueva versión.
    ///
    /// Escribe sobre `conn`, que puede ser un `&mut SqliteConnection` de pool o el
    /// de una transacción: la escritura de la fila de `rules` y la de sus
    /// condiciones comparten transacción en las tres vías de escritura.
    ///
    /// # Errors
    ///
    /// Devuelve un error de base de datos si el `DELETE` o algún `INSERT` fallan,
    /// y `AppError::Other` si el índice de una condición no cabe en el `position`
    /// `INTEGER` («demasiadas condiciones»).
    async fn write_conditions(
        conn: &mut SqliteConnection,
        rule: &Self,
        conditions: Option<&[Condition]>,
    ) -> Result<(), AppError> {
        // Las tres vías de escritura (creación, edición e importación) normalizan
        // la especificación de transformaciones **antes** de llegar aquí, así que
        // hoy este `unwrap_or_default` no degrada nada en la práctica. Es, no
        // obstante, una asimetría latente frente a `read_all_active`, que
        // **excluye** la regla cuando la especificación no se puede leer; si ese
        // contrato de entrada cambiara, esta escritura persistiría en silencio una
        // pipeline distinta de la que la carga usaría.
        let pipeline = TransformationPipeline::parse(&rule.transformations).unwrap_or_default();
        // El préstamo de la proyección necesita un dueño local; el conjunto
        // explícito de la autoría avanzada se usa tal cual.
        let mut projected = Vec::new();
        let conditions: &[Condition] = conditions.map_or_else(
            || {
                projected = condition::project(rule, &pipeline);
                projected.as_slice()
            },
            |list| list,
        );

        query("DELETE FROM rule_conditions WHERE rule_id = ?")
            .bind(rule.id)
            .execute(&mut *conn)
            .await?;

        for (index, condition) in conditions.iter().enumerate() {
            let position = i32::try_from(index)
                .map_err(|error| AppError::Other(format!("demasiadas condiciones: {error}")))?;
            let pattern = match &condition.operator {
                Operator::Rx(regex) => regex.as_str(),
                Operator::Truthy => "",
            };
            query(
                "INSERT INTO rule_conditions
                    (rule_id, position, collection, key, transformations, operator, pattern, negate)
                 VALUES (?, ?, ?, ?, ?, ?, ?, 0)",
            )
            .bind(rule.id)
            .bind(position)
            .bind(condition.collection.field_name())
            .bind(condition.key.as_deref())
            .bind(condition.pipeline.spec())
            .bind(condition.operator.name())
            .bind(pattern)
            .execute(&mut *conn)
            .await?;
        }

        Ok(())
    }

    /// Reemplaza las condiciones persistidas de la regla `rule_id`, leyendo su
    /// fila actual.
    ///
    /// Es la vía de la importación, que tras el UPSERT solo conoce el `id`: relee
    /// la fila y delega en [`Rule::write_conditions`]. Si `conditions` es `None`,
    /// proyecta desde los 14 campos releídos; si viene la lista compilada, la
    /// persiste tal cual. La creación y la edición, que ya tienen la fila en
    /// memoria, llaman directamente a `write_conditions`.
    ///
    /// # Errors
    ///
    /// Devuelve un error de base de datos si la lectura o la escritura fallan.
    pub(crate) async fn replace_conditions_for_id(
        conn: &mut SqliteConnection,
        rule_id: i32,
        conditions: Option<&[Condition]>,
    ) -> Result<(), AppError> {
        let row = query("SELECT * FROM rules WHERE id = ?")
            .bind(rule_id)
            .fetch_one(&mut *conn)
            .await?;
        let rule = Self::from_row(&row);
        Self::write_conditions(conn, &rule, conditions).await
    }

    /// Parsea una fila de `rule_conditions` en una [`Condition`], o `None` si la
    /// fila debe descartarse (colección desconocida, operador incoherente con su
    /// colección, patrón no compilable, especificación de transformaciones no
    /// legible o clave ausente en una colección que la exige).
    ///
    /// La columna `operator` reconstruye el operador: `@rx` (para las colecciones
    /// de texto) o `@truthy` (solo para `is_tor`). Una fila incoherente —`@truthy`
    /// en una colección de texto, `is_tor` con operador distinto de `@truthy`,
    /// `@truthy` con patrón, clave o transformaciones— se **descarta** con un
    /// diagnóstico que la nombra, en vez de aplicarse mal: es el fallo seguro
    /// frente al rollback a una imagen anterior, que no conoce `@truthy`.
    ///
    /// Es el criterio compartido por [`Rule::read_conditions`] y
    /// [`Rule::read_conditions_batch`]: una fila irresoluble se descarta con un
    /// `warn!`/`debug!` que la nombra en vez de tumbar la carga. Reutilizarlo
    /// evita que los dos caminos de lectura diverjan.
    fn condition_from_row(rule_id: i32, row: &SqliteRow) -> Option<Condition> {
        let collection_name: String = row.get("collection");
        let Some(collection) = Collection::from_field_name(&collection_name) else {
            debug!(
                rule_id,
                collection = %collection_name,
                "condición descartada: colección desconocida"
            );
            return None;
        };
        let operator_name: String = row.get("operator");
        let pattern: String = row.get("pattern");
        let transformations: String = row.get("transformations");
        let key: Option<String> = row.try_get("key").ok().flatten();

        let operator = match (collection, operator_name.as_str()) {
            (Collection::IsTor, "@truthy") => {
                if !pattern.is_empty() || key.is_some() || !transformations.is_empty() {
                    warn!(
                        rule_id,
                        collection = %collection_name,
                        "condición descartada: «@truthy» exige patrón vacío, sin clave y sin \
                         transformaciones"
                    );
                    return None;
                }
                Operator::Truthy
            },
            (Collection::IsTor, other) => {
                warn!(
                    rule_id,
                    collection = %collection_name,
                    operator = %other,
                    "condición descartada: la colección «is_tor» exige el operador «@truthy»"
                );
                return None;
            },
            (_, "@truthy") => {
                warn!(
                    rule_id,
                    collection = %collection_name,
                    "condición descartada: «@truthy» solo es válido para la colección «is_tor»"
                );
                return None;
            },
            (_, "@rx") => {
                let Ok(regex) = Regex::new(&pattern) else {
                    debug!(
                        rule_id,
                        collection = %collection_name,
                        "condición descartada: el patrón no compila"
                    );
                    return None;
                };
                Operator::Rx(regex)
            },
            (_, other) => {
                warn!(
                    rule_id,
                    collection = %collection_name,
                    operator = %other,
                    "condición descartada: operador desconocido"
                );
                return None;
            },
        };

        let pipeline = match TransformationPipeline::parse(&transformations) {
            Ok(pipeline) => pipeline,
            Err(error) => {
                warn!(
                    rule_id,
                    collection = %collection_name,
                    "condición descartada: la especificación de transformaciones no se puede leer: {error}"
                );
                return None;
            },
        };
        if collection.requires_key() && key.is_none() {
            warn!(
                rule_id,
                collection = %collection_name,
                "condición descartada: la colección exige clave y la fila no la trae"
            );
            return None;
        }
        Some(Condition {
            collection,
            key,
            pipeline,
            operator,
        })
    }

    /// Lee el conjunto ordenado de condiciones de `rule_id` desde
    /// `rule_conditions`, ordenado por `position ASC`.
    ///
    /// Es la inversa de [`Rule::write_conditions`]: parsea `collection` con
    /// [`Collection::from_field_name`], compila el `pattern` con `Regex::new` y
    /// parsea la `transformations` de la fila. Una fila con una colección
    /// desconocida, con un patrón que no compila, con una especificación de
    /// transformaciones no legible o con una clave ausente en una colección que
    /// la exige se **descarta** —con un `warn!` que la nombra— en vez de tumbar
    /// la carga: es el mismo criterio con el que [`condition::project`] no
    /// proyecta un patrón no compilable.
    ///
    /// # Errors
    ///
    /// Devuelve un error de base de datos si la consulta falla. Una fila
    /// descartada no es un error.
    pub(crate) async fn read_conditions(
        pool: &SqlitePool,
        rule_id: i32,
    ) -> Result<Vec<Condition>, Error> {
        let sql = "SELECT collection, key, transformations, operator, pattern
                   FROM rule_conditions WHERE rule_id = ? ORDER BY position ASC";
        let rows = query(sql).bind(rule_id).fetch_all(pool).await?;

        Ok(rows
            .iter()
            .filter_map(|row| Self::condition_from_row(rule_id, row))
            .collect())
    }

    /// Lee en **una sola consulta** las condiciones de todas las reglas de
    /// `rule_ids`, agrupadas por `rule_id` y ordenadas por `position ASC`.
    ///
    /// Es la vía del listado y la exportación: cargar las condiciones de cada
    /// regla con una consulta por regla sería un N+1. Una lista vacía devuelve un
    /// mapa vacío sin tocar la base de datos. El criterio de descarte de filas es
    /// el mismo que el de [`Rule::read_conditions`].
    ///
    /// # Errors
    ///
    /// Devuelve un error de base de datos si la consulta falla. Una fila
    /// descartada no es un error.
    pub(crate) async fn read_conditions_batch(
        pool: &SqlitePool,
        rule_ids: &[i32],
    ) -> Result<BTreeMap<i32, Vec<Condition>>, Error> {
        let mut grouped: BTreeMap<i32, Vec<Condition>> = BTreeMap::new();
        if rule_ids.is_empty() {
            return Ok(grouped);
        }

        // Los placeholders son dinámicos: una sola consulta con un `IN` del tamaño
        // del lote, nunca una consulta por regla.
        let placeholders = vec!["?"; rule_ids.len()].join(", ");
        let sql = format!(
            "SELECT rule_id, collection, key, transformations, operator, pattern
             FROM rule_conditions WHERE rule_id IN ({placeholders})
             ORDER BY rule_id, position ASC"
        );
        let mut query = query(&sql);
        for rule_id in rule_ids {
            query = query.bind(rule_id);
        }
        let rows = query.fetch_all(pool).await?;

        for row in &rows {
            let rule_id: i32 = row.get("rule_id");
            if let Some(condition) = Self::condition_from_row(rule_id, row) {
                grouped.entry(rule_id).or_default().push(condition);
            }
        }

        Ok(grouped)
    }

    fn from_row(row: &SqliteRow) -> Self {
        Self {
            id: row.get("id"),
            name: row.get("name"),
            description: row.get("description"),
            weight: row.get("weight"),
            mode: row.get("mode"),
            pipeline: row.get("pipeline"),
            transformations: row.get("transformations"),
            allow: row.get("allow"),
            is_tor: row.get("is_tor"),
            ip_address: row.get("ip_address"),
            protocol: row.get("protocol"),
            fqdn: row.get("fqdn"),
            path: row.get("path"),
            query: row.get("query"),
            city_name: row.get("city_name"),
            country_name: row.get("country_name"),
            country_code: row.get("country_code"),
            user_agent: row.get("user_agent"),
            method: row.get("method"),
            referer: row.get("referer"),
            content_type: row.get("content_type"),
            accept_language: row.get("accept_language"),
            x_request_id: row.get("x_request_id"),
            rate_limit_profile_id: row.try_get("rate_limit_profile_id").ok().flatten(),
            rate_limit_profile_name: row.try_get("rate_limit_profile_name").ok().flatten(),
            active: row.get("active"),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
        }
    }

    /// Creates a new rule in the database.
    ///
    /// El pipeline pasa por [`validate_pipeline`] —solo `waf` o `jail`— y el peso
    /// por [`normalized_weight`]: una regla de Jail se almacena siempre con peso 0,
    /// traiga lo que traiga la petición. La especificación de transformaciones pasa
    /// por [`normalized_transformations`], que la canoniza al escribirla: el
    /// marcador `none` no llega a la base de datos y la regla declara la pipeline
    /// que de verdad se aplica.
    ///
    /// # Errors
    ///
    /// Devuelve [`AppError::InvalidInput`] si el pipeline no es un pipeline
    /// admitido o si la especificación de transformaciones contiene un token que no
    /// se reconoce, y un error de base de datos si la consulta falla.
    pub async fn create(pool: &SqlitePool, rule: NewRule) -> Result<Self, AppError> {
        let sql =
            "INSERT INTO rules (name, description, weight, mode, pipeline, transformations, allow,
            is_tor, ip_address, protocol, fqdn, path, query, city_name, country_name,
            country_code, user_agent, method, referer, content_type,
            accept_language, x_request_id, rate_limit_profile_id,
            active, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?,
            ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?,
            ?, ?, ?, ?, ?, ?, ?) RETURNING *";
        let now = Utc::now();
        let pipeline = rule.pipeline.unwrap_or_else(|| WAF_PIPELINE.to_string());
        validate_pipeline(&pipeline)?;
        let transformations =
            normalized_transformations(rule.transformations.as_deref().unwrap_or_default())?;
        let weight = normalized_weight(&pipeline, rule.weight.unwrap_or(100));
        // Las condiciones se compilan y validan **antes** de abrir la transacción y
        // de insertar la fila: una regla con una condición inválida no debe
        // almacenarse (ni su fila ni sus condiciones). `None` significa modo simple.
        let compiled = compile_conditions(rule.conditions.as_deref())?;
        // La fila de `rules` y sus condiciones se escriben en la MISMA
        // transacción: una regla nunca queda almacenada sin el conjunto que
        // describe su matcheo.
        let mut tx = pool.begin().await?;
        let created = query(sql)
            .bind(&rule.name)
            .bind(rule.description.unwrap_or_default())
            .bind(weight)
            .bind(rule.mode.unwrap_or_else(|| "log_only".to_string()))
            .bind(pipeline)
            .bind(transformations)
            .bind(rule.allow.unwrap_or(true))
            .bind(rule.is_tor.unwrap_or(false))
            .bind(rule.ip_address)
            .bind(rule.protocol)
            .bind(rule.fqdn)
            .bind(rule.path)
            .bind(rule.query)
            .bind(rule.city_name)
            .bind(rule.country_name)
            .bind(rule.country_code)
            .bind(rule.user_agent)
            .bind(rule.method)
            .bind(rule.referer)
            .bind(rule.content_type)
            .bind(rule.accept_language)
            .bind(rule.x_request_id)
            .bind(rule.rate_limit_profile_id)
            .bind(rule.active.unwrap_or(true))
            .bind(now)
            .bind(now)
            .map(|row| Self::from_row(&row))
            .fetch_one(&mut *tx)
            .await?;
        Self::write_conditions(&mut tx, &created, compiled.as_deref()).await?;
        tx.commit().await?;
        Ok(created)
    }

    /// Reads rule info (total or active count).
    ///
    /// # Errors
    ///
    /// Returns an error if the database query fails.
    pub async fn read_info(pool: &SqlitePool, info: &str) -> Result<i64, Error> {
        let sql = if info == "total" {
            "SELECT count(*) FROM rules"
        } else if info == "active" {
            "SELECT count(*) FROM rules WHERE active = 1"
        } else {
            return Err(Error::RowNotFound);
        };
        query(sql)
            .map(|row: SqliteRow| row.get(0))
            .fetch_one(pool)
            .await
    }

    /// Reads all rule info counts.
    ///
    /// # Errors
    ///
    /// Returns an error if the database query fails.
    pub async fn read_info_all(pool: &SqlitePool) -> Result<RuleInfoCounts, Error> {
        let sql = "SELECT
            (SELECT count(*) FROM rules) as total,
            (SELECT count(*) FROM rules WHERE active = 1) as active";
        query(sql)
            .map(|row: SqliteRow| RuleInfoCounts {
                total: row.get("total"),
                active: row.get("active"),
            })
            .fetch_one(pool)
            .await
    }

    /// Updates an existing rule.
    ///
    /// El pipeline pasa por [`validate_pipeline`] —solo `waf` o `jail`— y el peso
    /// por [`normalized_weight`]: una regla de Jail sigue almacenada con peso 0
    /// aunque la petición envíe otro valor. La especificación de transformaciones
    /// pasa por [`normalized_transformations`], que la canoniza al escribirla: el
    /// marcador `none` no llega a la base de datos y la regla declara la pipeline
    /// que de verdad se aplica.
    ///
    /// # Errors
    ///
    /// Devuelve [`AppError::InvalidInput`] si el pipeline no es un pipeline
    /// admitido o si la especificación de transformaciones contiene un token que no
    /// se reconoce, y un error de base de datos si la consulta falla.
    pub async fn update(pool: &SqlitePool, mut rule: UpdateRule) -> Result<Self, AppError> {
        let sql = "UPDATE rules SET
                name = ?,
                description = ?,
                weight = ?,
                mode = ?,
                pipeline = ?,
                transformations = ?,
                allow = ?,
                is_tor = ?,
                ip_address = ?,
                protocol = ?,
                fqdn = ?,
                path = ?,
                query = ?,
                city_name = ?,
                country_name = ?,
                country_code = ?,
                user_agent = ?,
                method = ?,
                referer = ?,
                content_type = ?,
                accept_language = ?,
                x_request_id = ?,
                rate_limit_profile_id = ?,
                active = ?,
                updated_at = ?
            WHERE id = ?
            RETURNING *";
        let now = Utc::now();
        let pipeline = rule.pipeline;
        validate_pipeline(&pipeline)?;
        let transformations = normalized_transformations(&rule.transformations)?;
        let weight = normalized_weight(&pipeline, rule.weight);
        // Se compilan y validan **antes** del `UPDATE`: una edición con una
        // condición inválida no debe modificar la fila ni su conjunto. El `take`
        // evita el movimiento parcial del resto de campos de `rule`.
        let compiled = compile_conditions(rule.conditions.take().as_deref())?;
        // El `UPDATE` de la fila y el **reemplazo** de su conjunto de condiciones
        // comparten transacción: editar una regla deja la tabla de condiciones
        // con exactamente el conjunto de la nueva versión.
        let mut tx = pool.begin().await?;
        let updated = query(sql)
            .bind(&rule.name)
            .bind(&rule.description)
            .bind(weight)
            .bind(&rule.mode)
            .bind(pipeline)
            .bind(transformations)
            .bind(rule.allow)
            .bind(rule.is_tor)
            .bind(rule.ip_address)
            .bind(rule.protocol)
            .bind(rule.fqdn)
            .bind(rule.path)
            .bind(rule.query)
            .bind(rule.city_name)
            .bind(rule.country_name)
            .bind(rule.country_code)
            .bind(rule.user_agent)
            .bind(rule.method)
            .bind(rule.referer)
            .bind(rule.content_type)
            .bind(rule.accept_language)
            .bind(rule.x_request_id)
            .bind(rule.rate_limit_profile_id)
            .bind(rule.active)
            .bind(now)
            .bind(rule.id)
            .map(|row| Self::from_row(&row))
            .fetch_one(&mut *tx)
            .await?;
        Self::write_conditions(&mut tx, &updated, compiled.as_deref()).await?;
        tx.commit().await?;
        Ok(updated)
    }

    /// Counts rules with the given filters.
    ///
    /// # Errors
    ///
    /// Returns an error if the database query fails.
    #[allow(clippy::cast_possible_wrap)]
    pub async fn count_paged(pool: &SqlitePool, params: &ReadRuleParams) -> Result<i64, Error> {
        let like_filters = vec![
            ("ip_address", &params.ip_address),
            ("protocol", &params.protocol),
            ("fqdn", &params.fqdn),
            ("path", &params.path),
            ("query", &params.query),
            ("city_name", &params.city_name),
            ("country_name", &params.country_name),
            ("country_code", &params.country_code),
            ("name", &params.name),
            ("description", &params.description),
            ("mode", &params.mode),
        ];
        let active_like_filters: Vec<(&str, String)> = like_filters
            .into_iter()
            .filter_map(|(col, val)| val.as_ref().map(|v| (col, v.clone())))
            .collect();
        let mut sql = "SELECT COUNT(*) total FROM rules LEFT JOIN rate_limit_profiles ON rules.rate_limit_profile_id = rate_limit_profiles.id WHERE 1=1".to_string();
        for (col, _) in &active_like_filters {
            let _ = write!(sql, " AND {col} LIKE ?");
        }
        // Boolean exact-match filters
        if params.allow.is_some() {
            sql.push_str(" AND allow = ?");
        }
        if params.pipeline.is_some() {
            sql.push_str(" AND pipeline = ?");
        }
        if params.active.is_some() {
            sql.push_str(" AND active = ?");
        }
        // Integer exact-match filter
        if params.weight.is_some() {
            sql.push_str(" AND weight = ?");
        }
        // Joined table filter (rate_limit_profile_name)
        if params.rate_limit_profile_name.is_some() {
            sql.push_str(" AND rate_limit_profiles.name LIKE ?");
        }
        let mut query = query(&sql);
        for (_, value) in active_like_filters {
            query = query.bind(value);
        }
        if let Some(val) = params.allow {
            query = query.bind(val);
        }
        if let Some(ref val) = params.pipeline {
            query = query.bind(val);
        }
        if let Some(val) = params.active {
            query = query.bind(val);
        }
        if let Some(val) = params.weight {
            query = query.bind(val);
        }
        if let Some(ref val) = params.rate_limit_profile_name {
            query = query.bind(val);
        }
        query
            .map(|row: SqliteRow| {
                let count: i64 = row.get("total");
                count
            })
            .fetch_one(pool)
            .await
    }

    /// Reads rules with pagination and filters.
    ///
    /// # Errors
    ///
    /// Returns an error if the database query fails.
    #[allow(clippy::cast_possible_wrap)]
    pub async fn read_paged(
        pool: &SqlitePool,
        params: &ReadRuleParams,
    ) -> Result<Vec<Self>, Error> {
        let like_filters = vec![
            ("ip_address", &params.ip_address),
            ("protocol", &params.protocol),
            ("fqdn", &params.fqdn),
            ("path", &params.path),
            ("query", &params.query),
            ("city_name", &params.city_name),
            ("country_name", &params.country_name),
            ("country_code", &params.country_code),
            ("name", &params.name),
            ("description", &params.description),
            ("mode", &params.mode),
        ];
        let active_like_filters: Vec<(&str, String)> = like_filters
            .into_iter()
            .filter_map(|(col, val)| val.as_ref().map(|v| (col, v.clone())))
            .collect();
        let mut sql = "SELECT rules.*, rate_limit_profiles.name as rate_limit_profile_name FROM rules LEFT JOIN rate_limit_profiles ON rules.rate_limit_profile_id = rate_limit_profiles.id WHERE 1=1".to_string();
        for (col, _) in &active_like_filters {
            let _ = write!(sql, " AND {col} LIKE ?");
        }
        // Boolean exact-match filters
        if params.allow.is_some() {
            sql.push_str(" AND allow = ?");
        }
        if params.pipeline.is_some() {
            sql.push_str(" AND pipeline = ?");
        }
        if params.active.is_some() {
            sql.push_str(" AND active = ?");
        }
        // Integer exact-match filter
        if params.weight.is_some() {
            sql.push_str(" AND weight = ?");
        }
        // Joined table filter (rate_limit_profile_name)
        if let Some(ref _val) = params.rate_limit_profile_name {
            sql.push_str(" AND rate_limit_profiles.name LIKE ?");
        }
        let sort_by = params.sort_by.as_deref().unwrap_or("id");
        if [
            "id",
            "name",
            "description",
            "mode",
            "pipeline",
            "weight",
            "allow",
            "active",
            "ip_address",
            "protocol",
            "fqdn",
            "path",
            "city_name",
            "country_name",
            "country_code",
            "rate_limit_profile_name",
        ]
        .contains(&sort_by)
        {
            // El criterio pedido SHALL desempatar de forma determinista: sin desempate
            // SQLite puede devolver las filas empatadas en cualquier orden y, al paginar
            // con `LIMIT ? OFFSET ?`, una misma fila se repite en una página y desaparece
            // de la siguiente. La columna va cualificada (`rules.id`) porque la consulta
            // lleva el JOIN de `rate_limit_profiles`, que también tiene `id`.
            let direction = if params.asc.unwrap_or(true) {
                "ASC"
            } else {
                "DESC"
            };
            let _ = write!(sql, " ORDER BY {sort_by} {direction}, rules.id ASC");
        }
        sql.push_str(" LIMIT ? OFFSET ?");
        let mut query = query(&sql);
        for (_, value) in active_like_filters {
            query = query.bind(value);
        }
        if let Some(val) = params.allow {
            query = query.bind(val);
        }
        if let Some(ref val) = params.pipeline {
            query = query.bind(val);
        }
        if let Some(val) = params.active {
            query = query.bind(val);
        }
        if let Some(val) = params.weight {
            query = query.bind(val);
        }
        if let Some(ref val) = params.rate_limit_profile_name {
            query = query.bind(val);
        }
        let limit = params.limit.unwrap_or(DEFAULT_LIMIT) as i32;
        let page = params.page.unwrap_or(DEFAULT_PAGE).max(1);
        let offset = ((page - 1) as i32) * limit;
        query
            .bind(limit)
            .bind(offset)
            .map(|row| Self::from_row(&row))
            .fetch_all(pool)
            .await
    }

    /// Reads all rules.
    ///
    /// # Errors
    ///
    /// Returns an error if the database query fails.
    pub async fn read_all(pool: &SqlitePool) -> Result<Vec<Self>, Error> {
        let sql = "SELECT rules.*, rate_limit_profiles.name as rate_limit_profile_name FROM rules LEFT JOIN rate_limit_profiles ON rules.rate_limit_profile_id = rate_limit_profiles.id ORDER BY weight ASC";
        query(sql)
            .map(|row| Self::from_row(&row))
            .fetch_all(pool)
            .await
    }

    /// Reads a rule by ID.
    ///
    /// # Errors
    ///
    /// Returns an error if the database query fails.
    pub async fn read(pool: &SqlitePool, id: i32) -> Result<Self, Error> {
        let sql = "SELECT rules.*, rate_limit_profiles.name as rate_limit_profile_name FROM rules LEFT JOIN rate_limit_profiles ON rules.rate_limit_profile_id = rate_limit_profiles.id WHERE rules.id = ?";
        query(sql)
            .bind(id)
            .map(|row| Self::from_row(&row))
            .fetch_one(pool)
            .await
    }

    /// Deletes a rule by ID.
    ///
    /// # Errors
    ///
    /// Returns an error if the database query fails.
    pub async fn delete(pool: &SqlitePool, id: i32) -> Result<Self, Error> {
        let sql = "DELETE FROM rules WHERE id = ? RETURNING *";
        query(sql)
            .bind(id)
            .map(|row| Self::from_row(&row))
            .fetch_one(pool)
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::NewRequest;
    use crate::models::condition::{Collection, Condition, Operator};
    use crate::models::transformation::TransformationPipeline;
    use chrono::Utc;
    use std::borrow::Cow;
    use std::sync::{Arc, Mutex};

    fn make_request(is_tor: Option<bool>) -> NewRequest {
        NewRequest {
            ip_address: Some("8.8.8.8".to_string()),
            protocol: None,
            fqdn: None,
            path: None,
            query: None,
            city_name: None,
            country_name: None,
            country_code: None,
            user_agent: None,
            method: None,
            referer: None,
            content_type: None,
            accept_language: None,
            x_request_id: None,
            cookies: BTreeMap::new(),
            headers: BTreeMap::new(),
            query_args: BTreeMap::new(),
            basename: None,
            extension: None,
            is_tor,
            rule_id: None,
            created_at: Utc::now(),
        }
    }

    fn make_rule(is_tor: Option<bool>) -> CacheRule {
        let rule = Rule {
            id: 0,
            name: "test".to_string(),
            description: String::new(),
            weight: 100,
            mode: "enforce".to_string(),
            pipeline: "waf".to_string(),
            transformations: String::new(),
            allow: false,
            is_tor,
            ip_address: None,
            protocol: None,
            fqdn: None,
            path: None,
            query: None,
            city_name: None,
            country_name: None,
            country_code: None,
            user_agent: None,
            method: None,
            referer: None,
            content_type: None,
            accept_language: None,
            x_request_id: None,
            rate_limit_profile_id: None,
            rate_limit_profile_name: None,
            active: true,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        CacheRule::from_rule(&rule)
    }

    /// Escenario: rule.is_tor = Some(true), request.is_tor = Some(true) → matches
    #[test]
    fn test_matches_tor_filter_match() {
        let rule = make_rule(Some(true));
        let request = make_request(Some(true));
        assert!(rule.matches(&request));
    }

    /// Escenario: rule.is_tor = Some(true), request.is_tor = Some(false) → no match
    #[test]
    fn test_matches_tor_filter_no_match() {
        let rule = make_rule(Some(true));
        let request = make_request(Some(false));
        assert!(!rule.matches(&request));
    }

    /// Escenario: rule.is_tor = None, request.is_tor = Some(true) → matches (neutral)
    #[test]
    fn test_matches_tor_filter_neutral() {
        let rule = make_rule(None);
        let request = make_request(Some(true));
        assert!(rule.matches(&request));
    }

    /// Escenario: rule.is_tor = Some(true), request.is_tor = None → no match (unknown = not Tor)
    #[test]
    fn test_matches_tor_filter_lazy_unknown() {
        let rule = make_rule(Some(true));
        let request = make_request(None);
        assert!(!rule.matches(&request));
    }

    /// Escenario (migrado de `test_cache_rule_from_rule_copies_is_tor`): la
    /// columna `is_tor` de la regla se proyecta a una condición `is_tor`/`@truthy`
    /// en `CacheRule`, que es el modelo de condiciones que decide ahora.
    ///
    /// Intención preservada del test original: `Some(true)` produce el filtro Tor
    /// activo, mientras que `None` y `Some(false)` no producen condición alguna
    /// (neutrales).
    #[test]
    fn test_cache_rule_from_rule_projects_is_tor_condition() {
        let tor = make_rule(Some(true));
        assert!(
            tor.conditions
                .iter()
                .any(|condition| condition.collection == Collection::IsTor
                    && matches!(condition.operator, Operator::Truthy)),
            "`is_tor = Some(true)` debe proyectarse a una condición `is_tor`/`@truthy`; \
             condiciones: {:?}",
            tor.conditions
        );

        for neutral in [None, Some(false)] {
            let cache = make_rule(neutral);
            assert!(
                cache
                    .conditions
                    .iter()
                    .all(|condition| condition.collection != Collection::IsTor),
                "`is_tor = {neutral:?}` es neutral y no debe proyectar condición; \
                 condiciones: {:?}",
                cache.conditions
            );
        }
    }

    // ── missing-header-fields: matcher ──

    /// Construye una `CacheRule` desde la base de `make_rule` y aplica los
    /// filtros que indique `configure` antes de compilar los regex.
    fn make_rule_with(configure: impl FnOnce(&mut Rule)) -> CacheRule {
        let mut cache = make_rule(None);
        configure(&mut cache.rule);
        CacheRule::from_rule(&cache.rule)
    }

    /// Construye un `NewRequest` desde la base de `make_request` y aplica los
    /// campos que indique `configure`.
    fn make_request_with(configure: impl FnOnce(&mut NewRequest)) -> NewRequest {
        let mut request = make_request(None);
        configure(&mut request);
        request
    }

    /// Escenario: Restricted field absent makes the rule not match.
    /// Regla con regex de `user_agent` y petición sin `user_agent` → NO casa.
    ///
    /// El brazo `(Some(_), None)` de `condition_matches` devuelve `false`: la regla
    /// restringe el campo y la petición no lo trae.
    #[test]
    fn test_matches_restricted_field_absent_does_not_match() {
        let rule = make_rule_with(|r| r.user_agent = Some("curl".to_string()));
        let request = make_request(None); // user_agent ausente

        assert!(
            !rule.matches(&request),
            "una regla que restringe user_agent no puede casar una petición sin esa cabecera"
        );
    }

    /// Escenario: Restricted field present and matching.
    /// Guardián: decide el regex; debe seguir pasando tras el refactor.
    #[test]
    fn test_matches_restricted_field_present_and_matching() {
        let rule = make_rule_with(|r| r.user_agent = Some(r"curl/8\.5".to_string()));
        let request = make_request_with(|q| q.user_agent = Some("curl/8.5".to_string()));

        assert!(
            rule.matches(&request),
            "el regex debe casar cuando el valor está presente y coincide"
        );
    }

    /// Escenario: Restricted field present and not matching.
    /// Guardián: con el valor presente decide el regex; debe seguir pasando tras
    /// el refactor.
    #[test]
    fn test_matches_restricted_field_present_and_not_matching() {
        let rule = make_rule_with(|r| r.user_agent = Some(r"curl/8\.5".to_string()));
        let request = make_request_with(|q| q.user_agent = Some("Mozilla/5.0".to_string()));

        assert!(
            !rule.matches(&request),
            "un regex que no casa con el valor presente → la regla NO casa"
        );
    }

    /// Escenario: One absent field discards a rule that matches on the others.
    /// `path` casa pero `x_request_id` está ausente → la regla NO casa.
    ///
    /// El `&&` cortocircuita en el primer campo restringido y ausente: los demás
    /// no llegan a evaluarse y la regla se descarta.
    #[test]
    fn test_matches_absent_field_discards_rule_matching_others() {
        let rule = make_rule_with(|r| {
            r.path = Some(r"^/admin$".to_string());
            r.x_request_id = Some(r"^req-".to_string());
        });
        let request = make_request_with(|q| q.path = Some("/admin".to_string()));

        assert!(
            !rule.matches(&request),
            "un único campo restringido y ausente descarta la regla aunque los demás casen"
        );
    }

    /// Escenario: An unrestricted field is neutral.
    /// Guardián: la regla sólo restringe `path`; los campos que no restringe
    /// (`user_agent`, `referer`, `x_request_id`, ausentes) son neutrales → casa.
    /// Vigila que el refactor no toque la neutralidad de `(None, _)`.
    #[test]
    fn test_matches_unrestricted_fields_are_neutral() {
        let rule = make_rule_with(|r| r.path = Some(r"^/admin$".to_string()));
        let request = make_request_with(|q| q.path = Some("/admin".to_string()));

        assert!(
            rule.matches(&request),
            "los campos que la regla no restringe deben ser neutrales aunque falten"
        );
    }

    /// Escenario: A present but empty header counts as absent.
    /// `from_request` colapsa la cabecera vacía a `None` y la regla NO casa.
    ///
    /// El colapso a `None` ocurre en `from_request` y el matcher descarta la
    /// regla por el brazo `(Some(_), None)`.
    #[test]
    fn test_matches_present_but_empty_header_counts_as_absent() {
        let mut headers = http::HeaderMap::new();
        headers.insert(http::header::REFERER, http::HeaderValue::from_static(""));
        let request = NewRequest::from_request(&headers, None, None);

        assert!(
            request.referer.is_none(),
            "una cabecera presente pero vacía debe colapsar a None"
        );

        let rule = make_rule_with(|r| r.referer = Some("https://example.com".to_string()));
        assert!(
            !rule.matches(&request),
            "una cabecera vacía cuenta como ausente → la regla NO casa"
        );
    }

    // ── captura de logs (copia privada del patrón de `journald_reader`) ──

    #[derive(Clone, Default)]
    struct CapturedLog(Arc<Mutex<Vec<u8>>>);

    impl std::io::Write for CapturedLog {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.0
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .extend_from_slice(buf);
            Ok(buf.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    /// Ejecuta `f` con un subscriber a nivel DEBUG que graba cada evento y
    /// devuelve lo escrito. Es el único modo de observar el `debug!` del
    /// matcher.
    fn capture_debug<F: FnOnce()>(f: F) -> String {
        let sink = Arc::new(Mutex::new(Vec::new()));
        let writer = CapturedLog(Arc::clone(&sink));
        let subscriber = tracing_subscriber::fmt()
            .with_max_level(tracing::Level::DEBUG)
            .with_ansi(false)
            .without_time()
            .with_writer(move || CapturedLog(Arc::clone(&writer.0)))
            .finish();

        tracing::subscriber::with_default(subscriber, f);

        let bytes = sink
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();
        String::from_utf8_lossy(&bytes).to_string()
    }

    /// Escenario: Restricted field absent makes the rule not match (diagnóstico).
    /// Un match descartado por campo ausente emite un `debug!` que nombra el
    /// campo y la regla.
    ///
    /// No se fija el texto literal del mensaje: sólo que nombre el campo y algo
    /// que identifique la regla (su nombre o su id).
    #[test]
    fn test_matches_absent_field_emits_debug_diagnostic() {
        let rule = make_rule_with(|r| {
            r.id = 4242;
            r.name = "regla-diagnostico".to_string();
            r.user_agent = Some("curl".to_string());
        });
        let request = make_request(None);

        let log = capture_debug(|| {
            let _ = rule.matches(&request);
        });

        assert!(
            log.contains("user_agent"),
            "el diagnóstico debe nombrar el campo ausente; log capturado: {log:?}"
        );
        assert!(
            log.contains("regla-diagnostico") || log.contains("4242"),
            "el diagnóstico debe identificar la regla (nombre o id); log capturado: {log:?}"
        );
    }

    /// Escenario: A legitimate regex no-match is not diagnosed as an absent field.
    /// Con el valor presente y un regex que no casa NO se emite el `debug!` de
    /// campo ausente.
    ///
    /// Guardián: hoy pasa trivialmente (no hay diagnóstico alguno) y debe
    /// seguir pasando tras el refactor.
    #[test]
    fn test_matches_legitimate_regex_no_match_emits_no_diagnostic() {
        let rule = make_rule_with(|r| {
            r.id = 4242;
            r.name = "regla-diagnostico".to_string();
            r.user_agent = Some("curl".to_string());
        });
        let request = make_request_with(|q| q.user_agent = Some("Mozilla/5.0".to_string()));

        let log = capture_debug(|| {
            let _ = rule.matches(&request);
        });

        assert!(
            !log.contains("user_agent"),
            "no debe diagnosticar campo ausente cuando el valor está presente; log: {log:?}"
        );
        assert!(
            !log.contains("regla-diagnostico"),
            "no debe nombrar la regla en un no-match legítimo por regex; log: {log:?}"
        );
    }

    // ── waf-transformations: el matcher aplica la pipeline ──

    /// Escenario 1.7: una evasión codificada casa con la transformación.
    ///
    /// Una regla con `transformations: "urlDecodeUni"` y patrón `\.\./` casa una
    /// petición cuyo `path` es `%2e%2e%2f`, porque la pipeline decodifica el valor
    /// antes de compararlo.
    ///
    /// La **misma** regla con la pipeline vacía NO casa: ese segundo aserto es la
    /// medida del agujero actual —el valor crudo `%2e%2e%2f` no contiene `../`— y
    /// es lo que justifica la funcionalidad.
    #[test]
    fn test_transformation_url_decode_uni_matches_encoded_traversal() {
        let request = make_request_with(|q| q.path = Some("%2e%2e%2f".to_string()));

        let rule = make_rule_with(|r| {
            r.transformations = "urlDecodeUni".to_string();
            r.path = Some(r"\.\./".to_string());
        });
        assert!(
            rule.matches(&request),
            "con la pipeline `urlDecodeUni` el path `%2e%2e%2f` debe decodificarse \
             a `../` y casar `\\.\\./`"
        );

        let rule_without_pipeline = make_rule_with(|r| {
            r.path = Some(r"\.\./".to_string());
        });
        assert!(
            !rule_without_pipeline.matches(&request),
            "sin pipeline el valor crudo `%2e%2e%2f` no casa `\\.\\./`: es la evasión \
             que hoy pasa sin ser detectada"
        );
    }

    /// Escenario 1.8: una pipeline encadenada casa la inyección SQL codificada.
    ///
    /// `urlDecodeUni,compressWhitespace` decodifica `%20` a espacio y colapsa los
    /// huecos, de modo que `1%20union%20select%201` casa `union\s+select`.
    #[test]
    fn test_transformation_url_decode_uni_and_compress_whitespace_matches_sql_injection() {
        let rule = make_rule_with(|r| {
            r.transformations = "urlDecodeUni,compressWhitespace".to_string();
            r.path = Some(r"union\s+select".to_string());
        });
        let request = make_request_with(|q| q.path = Some("1%20union%20select%201".to_string()));

        assert!(
            rule.matches(&request),
            "tras `urlDecodeUni,compressWhitespace`, `1%20union%20select%201` debe \
             casar `union\\s+select`"
        );
    }

    /// Escenario 1.9: con la pipeline vacía nada cambia (caracterización).
    ///
    /// Es la garantía de que ninguna regla existente cambia de comportamiento: las
    /// dos evasiones de 1.7 y 1.8 NO casan con la pipeline vacía —igual que antes
    /// de existir las transformaciones— y la pipeline vacía no copia el valor.
    #[test]
    fn test_empty_pipeline_keeps_the_old_behaviour_and_borrows() {
        let traversal = make_request_with(|q| q.path = Some("%2e%2e%2f".to_string()));
        let rule_traversal = make_rule_with(|r| r.path = Some(r"\.\./".to_string()));
        assert!(
            !rule_traversal.matches(&traversal),
            "la evasión de path traversal no casa sin pipeline"
        );

        let sqli = make_request_with(|q| q.path = Some("1%20union%20select%201".to_string()));
        let rule_sqli = make_rule_with(|r| r.path = Some(r"union\s+select".to_string()));
        assert!(
            !rule_sqli.matches(&sqli),
            "la inyección SQL codificada no casa sin pipeline"
        );

        // La pipeline vacía es la de la regla sin transformaciones y no copia el
        // valor: `apply` devuelve `Cow::Borrowed`.
        assert!(
            rule_traversal
                .conditions
                .iter()
                .all(|c| c.pipeline.is_empty()),
            "una regla sin `transformations` tiene la pipeline vacía"
        );
        let pipeline =
            TransformationPipeline::parse("").expect("la cadena vacía es una pipeline válida");
        let value = "%2e%2e%2f";
        assert!(
            matches!(pipeline.apply(value), Cow::Borrowed(_)),
            "la pipeline vacía no debe copiar el valor"
        );
    }

    /// Escenario del delta `rule-matching`: «La pipeline no se aplica al patrón».
    ///
    /// El caso que lo demuestra es el único en el que transformar el patrón
    /// cambiaría el resultado: con `lowercase` el valor `/ADMIN` pasa a `/admin`,
    /// y el patrón `[A-Z]+` —comparado **tal como se escribió**— ya no casa. Si la
    /// pipeline se aplicase también al patrón, `[A-Z]+` habría pasado a `[a-z]+`
    /// y la regla habría casado. El control sin pipeline cierra el argumento: el
    /// mismo patrón contra el mismo valor crudo sí casa, así que la diferencia
    /// entre los dos casos es exactamente lo que la pipeline hizo —tocar el valor
    /// y no el patrón—.
    #[test]
    fn test_transformation_applies_to_request_value_not_to_pattern() {
        let with_pipeline = make_rule_with(|r| {
            r.transformations = "lowercase".to_string();
            r.path = Some("[A-Z]+".to_string());
        });
        let without_pipeline = make_rule_with(|r| r.path = Some("[A-Z]+".to_string()));
        let request = make_request_with(|q| q.path = Some("/ADMIN".to_string()));

        assert!(
            !with_pipeline.matches(&request),
            "el valor se transforma a `/admin` y el patrón `[A-Z]+` se compara tal \
             como se escribió: la regla NO puede casar"
        );
        assert!(
            without_pipeline.matches(&request),
            "sin pipeline el mismo patrón sí casa el valor crudo: si esto no casara, \
             el test no distinguiría el patrón transformado del no transformado"
        );
    }

    /// Escenario 1.11: un campo restringido y ausente sigue ausente.
    ///
    /// Con la pipeline activa, un campo que la regla restringe y que la petición no
    /// trae sigue sin casar y sigue emitiendo el `debug!` que lo nombra.
    #[test]
    fn test_absent_restricted_field_still_fails_and_diagnoses_with_pipeline() {
        let rule = make_rule_with(|r| {
            r.id = 4242;
            r.name = "regla-pipeline".to_string();
            r.transformations = "urlDecodeUni".to_string();
            r.user_agent = Some("curl".to_string());
        });
        let request = make_request(None); // user_agent ausente

        let mut matched = true;
        let log = capture_debug(|| {
            matched = rule.matches(&request);
        });

        assert!(
            !matched,
            "un campo restringido y ausente sigue sin casar con la pipeline activa"
        );
        assert!(
            log.contains("user_agent"),
            "el diagnóstico debe nombrar el campo ausente; log capturado: {log:?}"
        );
        assert!(
            log.contains("regla-pipeline") || log.contains("4242"),
            "el diagnóstico debe identificar la regla; log capturado: {log:?}"
        );
    }

    /// Escenario 1.12: la evidencia sale del mismo recorrido que decide el matcheo.
    ///
    /// - con la pipeline no vacía, `evaluate` devuelve por cada campo restringido
    ///   que casó el valor **transformado**;
    /// - con la pipeline vacía, `evidence` es `None`;
    /// - un campo que la regla no restringe nunca aparece en la evidencia.
    #[test]
    fn test_evaluate_evidence_contract() {
        // Caso 1: pipeline no vacía → evidencia con el valor transformado.
        let rule = make_rule_with(|r| {
            r.transformations = "lowercase".to_string();
            r.path = Some(r"(?i)^/admin$".to_string());
        });
        let request = make_request_with(|q| {
            q.path = Some("/ADMIN".to_string());
            q.user_agent = Some("curl".to_string());
        });
        let outcome = rule.evaluate(&request);

        assert!(
            outcome.matched,
            "el patrón insensible a mayúsculas casa `/ADMIN`"
        );
        assert!(
            outcome.evidence.is_some(),
            "con pipeline no vacía y un campo restringido que casó debe haber evidencia"
        );
        let evidence = outcome.evidence.unwrap_or_default();
        assert_eq!(
            evidence.get("path").map(String::as_str),
            Some("/admin"),
            "la evidencia debe llevar el valor transformado, no el crudo"
        );
        assert!(
            !evidence.contains_key("user_agent"),
            "un campo que la regla no restringe nunca aparece en la evidencia"
        );

        // Caso 2: pipeline vacía → sin evidencia.
        let rule_empty = make_rule_with(|r| r.path = Some(r"^/admin$".to_string()));
        let outcome_empty = rule_empty.evaluate(&make_request_with(|q| {
            q.path = Some("/admin".to_string());
        }));
        assert!(
            outcome_empty.matched,
            "la regla sin pipeline casa el valor crudo"
        );
        assert!(
            outcome_empty.evidence.is_none(),
            "sin pipeline no se emite evidencia"
        );
    }

    /// Una regla que **no** casa no emite evidencia, aunque algún campo evaluado
    /// antes del que falló haya casado.
    ///
    /// La cadena `&&` de [`CacheRule::evaluate`] cortocircuita: `path` se evalúa
    /// antes que `user_agent` y puede haber casado, pero la decisión final es
    /// «no casa» y la evidencia describe **esa** decisión. Sin este corte, un
    /// consumidor que registrase la evidencia sin mirar `matched` afirmaría que
    /// `path` casó en una petición que se descartó.
    ///
    /// El segundo tramo repite la evaluación con el `user_agent` presente —y
    /// casando— para que el aserto no pueda pasar por la razón equivocada: la
    /// misma petición **sí** casa y **sí** emite la evidencia de `path` cuando no
    /// hay ningún campo que falle.
    #[test]
    fn test_evaluate_no_match_emits_no_evidence() {
        // `path` casa tras `lowercase` (`/ADMIN` → `/admin`), pero `user_agent`
        // está restringido y ausente: la regla NO casa.
        let rule = make_rule_with(|r| {
            r.transformations = "lowercase".to_string();
            r.path = Some(r"^/admin$".to_string());
            r.user_agent = Some("curl".to_string());
        });
        let request = make_request_with(|q| {
            q.path = Some("/ADMIN".to_string());
            q.user_agent = None;
        });
        let outcome = rule.evaluate(&request);

        assert!(
            !outcome.matched,
            "la regla restringe `user_agent` y la petición no lo trae: NO casa"
        );
        assert!(
            outcome.evidence.is_none(),
            "una regla que no casa no debe emitir evidencia, aunque `path` haya \
             casado antes de fallar `user_agent`; evidencia: {:?}",
            outcome.evidence
        );

        // El `path` SÍ habría casado: con `user_agent` presente la regla casa y
        // emite la evidencia de `path` con el valor transformado.
        let request_with_ua = make_request_with(|q| {
            q.path = Some("/ADMIN".to_string());
            q.user_agent = Some("curl".to_string());
        });
        let outcome_with_ua = rule.evaluate(&request_with_ua);

        assert!(
            outcome_with_ua.matched,
            "con `user_agent` presente y casando, la regla casa"
        );
        let evidence = outcome_with_ua.evidence.unwrap_or_default();
        assert_eq!(
            evidence.get("path").map(String::as_str),
            Some("/admin"),
            "una regla que casa sí emite la evidencia de `path` con el valor transformado"
        );
    }

    // ── waf-condition-ir (F2a): caracterización del contrato que la proyección
    // a condiciones debe preservar ──
    //
    // Estos tests fijan la equivalencia observable de `evaluate()` sobre los 14
    // campos: mismo `matched` y misma forma de la evidencia. Verifican el contrato
    // de la proyección.

    /// Construye una `CacheRule` que restringe **solo** la colección indicada,
    /// con el patrón `^match-me$` y la pipeline `lowercase`.
    fn rule_restricting(collection: Collection) -> CacheRule {
        make_rule_with(|r| {
            r.transformations = "lowercase".to_string();
            let pattern = Some("^match-me$".to_string());
            match collection {
                Collection::IpAddress => r.ip_address = pattern,
                Collection::Protocol => r.protocol = pattern,
                Collection::Fqdn => r.fqdn = pattern,
                Collection::Path => r.path = pattern,
                Collection::Query => r.query = pattern,
                Collection::CityName => r.city_name = pattern,
                Collection::CountryName => r.country_name = pattern,
                Collection::CountryCode => r.country_code = pattern,
                Collection::UserAgent => r.user_agent = pattern,
                Collection::Method => r.method = pattern,
                Collection::Referer => r.referer = pattern,
                Collection::ContentType => r.content_type = pattern,
                Collection::AcceptLanguage => r.accept_language = pattern,
                Collection::RequestId => r.x_request_id = pattern,
                // Las cinco colecciones nuevas y la booleana no se proyectan
                // desde los 14 campos del modo simple; este helper de
                // caracterización solo recorre `Collection::ALL`, así que no las
                // toca.
                Collection::RequestCookies
                | Collection::RequestHeaders
                | Collection::ArgsGet
                | Collection::RequestBasename
                | Collection::RequestExtension
                | Collection::IsTor => {},
            }
        })
    }

    /// Construye un `NewRequest` que trae la colección indicada con el valor dado.
    fn request_carrying(collection: Collection, value: &str) -> NewRequest {
        make_request_with(|q| {
            let value = Some(value.to_string());
            match collection {
                Collection::IpAddress => q.ip_address = value,
                Collection::Protocol => q.protocol = value,
                Collection::Fqdn => q.fqdn = value,
                Collection::Path => q.path = value,
                Collection::Query => q.query = value,
                Collection::CityName => q.city_name = value,
                Collection::CountryName => q.country_name = value,
                Collection::CountryCode => q.country_code = value,
                Collection::UserAgent => q.user_agent = value,
                Collection::Method => q.method = value,
                Collection::Referer => q.referer = value,
                Collection::ContentType => q.content_type = value,
                Collection::AcceptLanguage => q.accept_language = value,
                Collection::RequestId => q.x_request_id = value,
                // Las colecciones nuevas se dejan con una entrada fija de
                // clave-valor para que la resolución real por clave funcione.
                Collection::RequestCookies => {
                    q.cookies.insert("clave".to_string(), "valor".to_string());
                },
                Collection::RequestHeaders => {
                    q.headers.insert("clave".to_string(), "valor".to_string());
                },
                Collection::ArgsGet => {
                    q.query_args
                        .insert("clave".to_string(), "valor".to_string());
                },
                Collection::RequestBasename => q.basename = value,
                Collection::RequestExtension => q.extension = value,
                // La colección booleana se resuelve desde `NewRequest::is_tor`.
                Collection::IsTor => q.is_tor = Some(true),
            }
        })
    }

    /// caracterización: verifica el comportamiento final.
    ///
    /// Escenario del delta: «La proyección de los 14 campos a condiciones es
    /// total y equivalente». Para cada uno de los 14 campos, una regla que lo
    /// restringe casa la petición que lo trae con el valor correcto —y emite su
    /// evidencia bajo el nombre del campo— y no casa la que lo trae con otro.
    #[test]
    fn test_evaluate_is_equivalent_over_the_fourteen_fields() {
        for collection in Collection::ALL {
            let rule = rule_restricting(collection);

            let matching = request_carrying(collection, "MATCH-ME");
            let outcome = rule.evaluate(&matching);
            assert!(
                outcome.matched,
                "la regla que restringe {collection:?} debe casar la petición que la trae"
            );
            let evidence = outcome.evidence.expect(
                "con pipeline no vacía y un campo restringido que casa debe haber evidencia",
            );
            assert_eq!(
                evidence.get(collection.field_name()).map(String::as_str),
                Some("match-me"),
                "la evidencia se anota bajo el nombre del campo, con el valor transformado"
            );

            let non_matching = request_carrying(collection, "otro");
            let outcome = rule.evaluate(&non_matching);
            assert!(
                !outcome.matched,
                "la regla que restringe {collection:?} no debe casar otro valor"
            );
            assert!(
                outcome.evidence.is_none(),
                "una regla que no casa no emite evidencia"
            );
        }
    }

    /// caracterización: verifica el comportamiento final.
    ///
    /// Regla que restringe `user_agent` con una petición sin `user_agent` → NO
    /// casa y no hay evidencia. Es el brazo `(Some(_), None)` del matcheo que la
    /// condición debe preservar.
    #[test]
    fn test_restricted_absent_field_does_not_match_and_has_no_evidence() {
        let rule = make_rule_with(|r| r.user_agent = Some("curl".to_string()));
        let request = make_request(None); // user_agent ausente

        let outcome = rule.evaluate(&request);

        assert!(
            !outcome.matched,
            "una regla que restringe `user_agent` no casa una petición sin esa cabecera"
        );
        assert!(
            outcome.evidence.is_none(),
            "sin match no hay evidencia; evidencia: {:?}",
            outcome.evidence
        );
    }

    /// caracterización: verifica el comportamiento final.
    ///
    /// La evidencia se anota bajo el nombre del campo con el valor
    /// **transformado**; con la pipeline vacía no hay evidencia.
    #[test]
    fn test_evidence_key_is_the_field_name_and_value_is_transformed() {
        let rule = make_rule_with(|r| {
            r.transformations = "lowercase".to_string();
            r.path = Some("^/admin$".to_string());
        });
        let request = make_request_with(|q| q.path = Some("/ADMIN".to_string()));

        let outcome = rule.evaluate(&request);
        assert!(
            outcome.matched,
            "tras `lowercase`, `/ADMIN` casa `^/admin$`"
        );
        let evidence = outcome.evidence.expect("debe haber evidencia");
        assert_eq!(
            evidence.get("path").map(String::as_str),
            Some("/admin"),
            "la evidencia se anota bajo `path` con el valor transformado"
        );

        let rule_empty = make_rule_with(|r| r.path = Some("^/admin$".to_string()));
        let outcome_empty = rule_empty.evaluate(&make_request_with(|q| {
            q.path = Some("/admin".to_string());
        }));
        assert!(outcome_empty.matched, "sin pipeline casa el valor crudo");
        assert!(
            outcome_empty.evidence.is_none(),
            "con la pipeline vacía no se emite evidencia"
        );
    }

    /// caracterización: verifica el comportamiento final.
    ///
    /// `is_tor` es una condición más del conjunto, evaluada en AND con las de los
    /// campos: una regla con `is_tor` y `path` casa solo si el path casa **y** la
    /// petición es Tor.
    #[test]
    fn test_is_tor_is_a_condition_evaluated_with_the_others() {
        let rule = make_rule_with(|r| {
            r.is_tor = Some(true);
            r.path = Some("^/admin$".to_string());
        });

        let non_tor = make_request_with(|q| {
            q.path = Some("/admin".to_string());
            q.is_tor = Some(false);
        });
        assert!(
            !rule.matches(&non_tor),
            "con `is_tor = Some(true)` una petición no-Tor no casa aunque `path` case"
        );

        let tor = make_request_with(|q| {
            q.path = Some("/admin".to_string());
            q.is_tor = Some(true);
        });
        assert!(
            rule.matches(&tor),
            "la misma petición marcada como Tor sí casa"
        );
    }

    // ── F2b-3: matcheo de la colección booleana `is_tor` ──

    /// Construye una `CacheRule` con un conjunto de condiciones **explícito**, sin
    /// proyectar los campos de la regla.
    fn make_rule_with_conditions(conditions: Vec<Condition>) -> CacheRule {
        let mut cache = make_rule(None);
        cache.conditions = conditions;
        cache
    }

    /// La condición booleana canónica de la colección `is_tor`.
    fn tor_condition() -> Condition {
        Condition {
            collection: Collection::IsTor,
            key: None,
            pipeline: TransformationPipeline::default(),
            operator: Operator::Truthy,
        }
    }

    /// Escenario del delta: «Una petición de Tor cumple la condición» y sus
    /// hermanos —`Some(false)` y `None` NO cumplen—.
    #[test]
    fn test_is_tor_condition_resolves_the_request_boolean() {
        let rule = make_rule_with_conditions(vec![tor_condition()]);

        assert!(
            rule.matches(&make_request(Some(true))),
            "`@truthy` cumple cuando el booleano de la petición es `true`"
        );
        assert!(
            !rule.matches(&make_request(Some(false))),
            "`@truthy` NO cumple con `Some(false)`"
        );
        assert!(
            !rule.matches(&make_request(None)),
            "`@truthy` NO cumple con `None`: la ausencia del dato es fail-open"
        );
    }

    /// Escenario del delta: «La ausencia del dato es fail-open y diagnostica».
    #[test]
    fn test_is_tor_absence_emits_debug_naming_collection_and_rule() {
        let mut rule = make_rule_with_conditions(vec![tor_condition()]);
        rule.rule.id = 4242;
        rule.rule.name = "regla-tor-diagnostico".to_string();

        let log = capture_debug(|| {
            let _ = rule.matches(&make_request(None));
        });

        assert!(
            log.contains("is_tor"),
            "el diagnóstico debe nombrar la colección; log capturado: {log:?}"
        );
        assert!(
            log.contains("regla-tor-diagnostico") || log.contains("4242"),
            "el diagnóstico debe identificar la regla; log capturado: {log:?}"
        );
    }

    /// Escenario del delta: «La colección booleana no anota evidencia».
    #[test]
    fn test_is_tor_condition_does_not_annotate_evidence() {
        let path_condition = Condition {
            collection: Collection::Path,
            key: None,
            pipeline: TransformationPipeline::parse("lowercase").unwrap_or_default(),
            operator: Operator::Rx(Regex::new(r"^/admin/\d+$").unwrap()),
        };
        let rule = make_rule_with_conditions(vec![path_condition, tor_condition()]);
        let request = make_request_with(|q| {
            q.path = Some("/ADMIN/42".to_string());
            q.is_tor = Some(true);
        });

        let outcome = rule.evaluate(&request);
        assert!(
            outcome.matched,
            "la petición Tor con `path` casado cumple la regla"
        );
        let evidence = outcome.evidence.expect("debe haber evidencia de `path`");
        assert!(
            evidence.contains_key("path"),
            "la evidencia de `path` debe existir; evidencia: {evidence:?}"
        );
        assert!(
            !evidence.contains_key("is_tor"),
            "la colección booleana NO debe anotar evidencia; evidencia: {evidence:?}"
        );
    }

    // ── F2b-3: validación de la autoría avanzada ──

    /// Escenario: «Una condición de `is_tor` con `@truthy` se acepta».
    #[test]
    fn test_compile_accepts_is_tor_truthy() {
        let input = ConditionInput {
            collection: "is_tor".to_string(),
            key: None,
            transformations: String::new(),
            operator: Some("@truthy".to_string()),
            pattern: String::new(),
        };

        let condition = input.compile().expect("`is_tor`/`@truthy` debe aceptarse");

        assert_eq!(condition.collection, Collection::IsTor);
        assert!(
            matches!(condition.operator, Operator::Truthy),
            "la condición compilada debe llevar el operador booleano"
        );
        assert!(condition.key.is_none(), "no admite clave");
        assert!(condition.pipeline.is_empty(), "no admite transformaciones");
    }

    /// Escenario: «`@truthy` en una colección de texto se rechaza (400)».
    #[test]
    fn test_compile_rejects_truthy_on_text_collection() {
        let input = ConditionInput {
            collection: "path".to_string(),
            key: None,
            transformations: String::new(),
            operator: Some("@truthy".to_string()),
            pattern: "^/admin$".to_string(),
        };

        assert!(
            input.compile().is_err(),
            "`@truthy` fuera de `is_tor` debe rechazarse"
        );
    }

    /// Escenario: «`@rx` sobre `is_tor` se rechaza (400)».
    #[test]
    fn test_compile_rejects_rx_on_is_tor() {
        let input = ConditionInput {
            collection: "is_tor".to_string(),
            key: None,
            transformations: String::new(),
            operator: Some("@rx".to_string()),
            pattern: "true".to_string(),
        };

        assert!(
            input.compile().is_err(),
            "`@rx` sobre `is_tor` debe rechazarse"
        );
    }

    /// Una condición de `is_tor` con `@truthy` exige patrón vacío, sin clave y sin
    /// transformaciones.
    #[test]
    fn test_compile_rejects_truthy_with_pattern_key_or_transformations() {
        let with_pattern = ConditionInput {
            collection: "is_tor".to_string(),
            key: None,
            transformations: String::new(),
            operator: Some("@truthy".to_string()),
            pattern: "true".to_string(),
        };
        assert!(
            with_pattern.compile().is_err(),
            "`@truthy` exige patrón vacío"
        );

        let with_key = ConditionInput {
            collection: "is_tor".to_string(),
            key: Some("k".to_string()),
            transformations: String::new(),
            operator: Some("@truthy".to_string()),
            pattern: String::new(),
        };
        assert!(with_key.compile().is_err(), "`@truthy` no admite clave");

        let with_transformations = ConditionInput {
            collection: "is_tor".to_string(),
            key: None,
            transformations: "lowercase".to_string(),
            operator: Some("@truthy".to_string()),
            pattern: String::new(),
        };
        assert!(
            with_transformations.compile().is_err(),
            "`@truthy` no admite transformaciones"
        );
    }
}
