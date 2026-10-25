//! # IR de condiciones del matcheo (F2a)
//!
//! Representación interna con la que el matcher describe una regla: un
//! **conjunto ordenado de condiciones**. Cada condición declara la colección de
//! la petición que restringe, la pipeline de transformaciones que se aplica a su
//! valor antes de compararlo y el operador con el que se compara.
//!
//! Desde F2b-1 la fuente persistida del matcheo es la tabla `rule_conditions`:
//! el matcher **lee** de ella el conjunto ordenado de condiciones. Los 14 campos
//! de [`Rule`] siguen siendo el origen del que `project` deriva ese conjunto, pero
//! solo al **escribir** ([`crate::models::Rule::write_conditions`]), que proyecta
//! los campos a condiciones y las persiste; la lectura
//! ([`crate::models::Rule::read_conditions`]) reconstruye la IR desde la tabla.
//! `project` es, pues, la proyección que alimenta la tabla, no la que la sustituye.

use crate::models::transformation::TransformationPipeline;
use crate::models::{NewRequest, Rule};
use regex::Regex;

/// Colección de la petición que una condición restringe.
///
/// Las 20 variantes se dividen en tres grupos:
/// - las 14 de campo, que corresponden, una a una, con los 14 campos de [`Rule`]
///   y de [`NewRequest`];
/// - las 5 nuevas (`RequestCookies`, `RequestHeaders`, `ArgsGet`,
///   `RequestBasename`, `RequestExtension`), que se resuelven desde partes
///   específicas de la petición y no desde los campos del modo simple;
/// - la colección booleana `IsTor`, que no se resuelve por
///   [`Collection::request_value`] (que devuelve texto) sino desde el booleano
///   [`NewRequest::is_tor`]. **NO** forma parte de [`Collection::ALL`], que sigue
///   siendo las 14 de campo: es la lista de la representación simple y del orden
///   histórico, no el catálogo completo.
///
/// Las colecciones con clave (cookies, cabeceras y argumentos del query)
/// exigen el nombre del elemento; las demás no admiten clave. La relación con
/// [`NewRequest`] es directa: cada variante sabe leer su valor de la petición
/// mediante [`Collection::request_value`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Collection {
    IpAddress,
    Protocol,
    Fqdn,
    Path,
    Query,
    CityName,
    CountryName,
    CountryCode,
    UserAgent,
    Method,
    Referer,
    ContentType,
    AcceptLanguage,
    RequestId,
    /// Cookies de la petición; exige clave (el nombre de la cookie).
    RequestCookies,
    /// Cabeceras de la petición; exige clave (el nombre en minúsculas).
    RequestHeaders,
    /// Argumentos del query string; exige clave (el nombre del parámetro).
    ArgsGet,
    /// Último segmento no vacío del path; no admite clave.
    RequestBasename,
    /// Extensión del basename; no admite clave.
    RequestExtension,
    /// La colección booleana: se resuelve desde [`NewRequest::is_tor`], no desde
    /// [`Collection::request_value`]. No admite clave y no está en
    /// [`Collection::ALL`].
    IsTor,
}

impl Collection {
    /// Las 14 colecciones, en el MISMO orden que la cadena `&&` de
    /// [`crate::models::CacheRule::evaluate`].
    pub const ALL: [Self; 14] = [
        Self::IpAddress,
        Self::Protocol,
        Self::Fqdn,
        Self::Path,
        Self::Query,
        Self::CityName,
        Self::CountryName,
        Self::CountryCode,
        Self::UserAgent,
        Self::Method,
        Self::Referer,
        Self::ContentType,
        Self::AcceptLanguage,
        Self::RequestId,
    ];

    /// El nombre del campo tal como la evidencia lo usa HOY (`"path"`,
    /// `"user_agent"`…).
    ///
    /// NO es vocabulario de colección: es exactamente la clave con la que el
    /// evento de auditoría anota el valor transformado, y renombrarla sería un
    /// cambio observable que esta fase no hace.
    #[must_use]
    pub const fn field_name(self) -> &'static str {
        match self {
            Self::IpAddress => "ip_address",
            Self::Protocol => "protocol",
            Self::Fqdn => "fqdn",
            Self::Path => "path",
            Self::Query => "query",
            Self::CityName => "city_name",
            Self::CountryName => "country_name",
            Self::CountryCode => "country_code",
            Self::UserAgent => "user_agent",
            Self::Method => "method",
            Self::Referer => "referer",
            Self::ContentType => "content_type",
            Self::AcceptLanguage => "accept_language",
            Self::RequestId => "x_request_id",
            Self::RequestCookies => "request_cookies",
            Self::RequestHeaders => "request_headers",
            Self::ArgsGet => "args_get",
            Self::RequestBasename => "request_basename",
            Self::RequestExtension => "request_extension",
            Self::IsTor => "is_tor",
        }
    }

    /// Indica si la colección exige clave (el nombre de la cookie, cabecera o
    /// argumento) para resolver su valor.
    #[must_use]
    pub const fn requires_key(self) -> bool {
        matches!(
            self,
            Self::RequestCookies | Self::RequestHeaders | Self::ArgsGet
        )
    }

    /// La colección cuyo [`Collection::field_name`] es `name`.
    ///
    /// Es el inverso exacto de [`Collection::field_name`]: la proyección desde la
    /// tabla `rule_conditions` guarda el nombre del campo, y esta función lo
    /// traduce de vuelta a la variante. Un nombre desconocido —una fila escrita
    /// por una versión con más colecciones— devuelve `None`, de modo que quien lee
    /// puede descartar la fila en vez de inventar una colección que no existe.
    #[must_use]
    pub fn from_field_name(name: &str) -> Option<Self> {
        Some(match name {
            "ip_address" => Self::IpAddress,
            "protocol" => Self::Protocol,
            "fqdn" => Self::Fqdn,
            "path" => Self::Path,
            "query" => Self::Query,
            "city_name" => Self::CityName,
            "country_name" => Self::CountryName,
            "country_code" => Self::CountryCode,
            "user_agent" => Self::UserAgent,
            "method" => Self::Method,
            "referer" => Self::Referer,
            "content_type" => Self::ContentType,
            "accept_language" => Self::AcceptLanguage,
            "x_request_id" => Self::RequestId,
            "request_cookies" => Self::RequestCookies,
            "request_headers" => Self::RequestHeaders,
            "args_get" => Self::ArgsGet,
            "request_basename" => Self::RequestBasename,
            "request_extension" => Self::RequestExtension,
            "is_tor" => Self::IsTor,
            _ => return None,
        })
    }

    /// El valor de la petición para esta colección, o `None` si no la trae.
    ///
    /// Recibe la clave opcional de la condición. Las colecciones con clave
    /// (`RequestCookies`, `RequestHeaders`, `ArgsGet`) la usan para resolver
    /// su valor; las colecciones sin clave ignoran el argumento. El brazo de
    /// `RequestHeaders` normaliza la clave a minúsculas como defensa: aunque
    /// `ConditionInput::compile` ya lo hace, una condición construida a mano con
    /// mayúsculas también debe casar.
    #[must_use]
    pub fn request_value<'a>(
        self,
        request: &'a NewRequest,
        key: Option<&str>,
    ) -> Option<&'a String> {
        match self {
            Self::IpAddress => request.ip_address.as_ref(),
            Self::Protocol => request.protocol.as_ref(),
            Self::Fqdn => request.fqdn.as_ref(),
            Self::Path => request.path.as_ref(),
            Self::Query => request.query.as_ref(),
            Self::CityName => request.city_name.as_ref(),
            Self::CountryName => request.country_name.as_ref(),
            Self::CountryCode => request.country_code.as_ref(),
            Self::UserAgent => request.user_agent.as_ref(),
            Self::Method => request.method.as_ref(),
            Self::Referer => request.referer.as_ref(),
            Self::ContentType => request.content_type.as_ref(),
            Self::AcceptLanguage => request.accept_language.as_ref(),
            Self::RequestId => request.x_request_id.as_ref(),
            Self::RequestCookies => key.and_then(|k| request.cookies.get(k)),
            Self::RequestHeaders => key.and_then(|k| request.headers.get(&k.to_lowercase())),
            Self::ArgsGet => key.and_then(|k| request.query_args.get(k)),
            Self::RequestBasename => request.basename.as_ref(),
            Self::RequestExtension => request.extension.as_ref(),
            // La colección booleana no se resuelve por `request_value`, que
            // devuelve texto: la resuelve el matcheo desde `NewRequest::is_tor`.
            Self::IsTor => None,
        }
    }

    /// El patrón de la regla para esta colección, o `None` si no la restringe.
    ///
    /// Es la lectura simétrica de [`Collection::request_value`]: aquella lee el
    /// valor que trae la petición, esta el patrón con el que la regla restringe
    /// el campo. Una colección sin patrón (`None`) no genera condición: es el
    /// brazo neutral del matcheo.
    #[must_use]
    pub const fn rule_pattern(self, rule: &Rule) -> Option<&String> {
        match self {
            Self::IpAddress => rule.ip_address.as_ref(),
            Self::Protocol => rule.protocol.as_ref(),
            Self::Fqdn => rule.fqdn.as_ref(),
            Self::Path => rule.path.as_ref(),
            Self::Query => rule.query.as_ref(),
            Self::CityName => rule.city_name.as_ref(),
            Self::CountryName => rule.country_name.as_ref(),
            Self::CountryCode => rule.country_code.as_ref(),
            Self::UserAgent => rule.user_agent.as_ref(),
            Self::Method => rule.method.as_ref(),
            Self::Referer => rule.referer.as_ref(),
            Self::ContentType => rule.content_type.as_ref(),
            Self::AcceptLanguage => rule.accept_language.as_ref(),
            Self::RequestId => rule.x_request_id.as_ref(),
            // Las cinco colecciones nuevas no se proyectan desde los 14 campos
            // del modo simple, así que nunca tienen patrón en una `Rule`. La
            // columna `is_tor` se proyecta como su propia condición booleana, no
            // como patrón de texto: nunca tiene patrón.
            Self::RequestCookies
            | Self::RequestHeaders
            | Self::ArgsGet
            | Self::RequestBasename
            | Self::RequestExtension
            | Self::IsTor => None,
        }
    }
}

/// Operador de comparación de una condición.
///
/// `Rx` compara el valor transformado con un regex. `Truthy` es el operador
/// booleano: se cumple **si y solo si** el booleano de la petición es
/// exactamente `true`.
#[derive(Debug, Clone)]
pub enum Operator {
    /// Compara el valor transformado con un regex.
    Rx(Regex),
    /// El operador booleano de la colección `is_tor`: cumple si y solo si el
    /// booleano de la petición es `true`.
    Truthy,
}

impl Operator {
    /// El nombre canónico con el que el operador se persiste en la columna
    /// `operator` de `rule_conditions` y se serializa en la API: `@rx` para
    /// [`Operator::Rx`] y `@truthy` para [`Operator::Truthy`].
    ///
    /// Es la representación que la escritura guarda y la lectura reconoce, de
    /// modo que el round-trip de la condición es estable.
    #[must_use]
    pub const fn name(&self) -> &'static str {
        match self {
            Self::Rx(_) => "@rx",
            Self::Truthy => "@truthy",
        }
    }
}

/// Una condición ya compilada.
///
/// Describe la colección que restringe, la clave opcional (obligatoria
/// exactamente para las colecciones con clave), la pipeline que se aplica a su
/// valor y el operador con el que se compara.
#[derive(Debug, Clone)]
pub struct Condition {
    pub collection: Collection,
    /// El nombre de la cookie, la cabecera o el argumento para las colecciones
    /// con clave; `None` para las que no la admiten.
    pub key: Option<String>,
    pub pipeline: TransformationPipeline,
    pub operator: Operator,
}

/// Proyecta la representación simple de una regla a su conjunto ordenado de
/// condiciones.
///
/// Recorre [`Collection::ALL`] **en su orden** y proyecta **una** condición por
/// cada campo que la regla restrinja con un patrón **no vacío y compilable**. Un
/// campo sin patrón (`None`), en blanco o cuyo patrón no compile no proyecta
/// nada: su ausencia equivale al brazo `(None, _)` neutral del matcheo.
///
/// La columna `is_tor` de la regla se proyecta como **una** condición de más:
/// cuando `is_tor == Some(true)` añade **al final** una condición
/// `{ IsTor, key: None, pipeline vacía, Operator::Truthy }`, después de las
/// condiciones de los 14 campos. `Some(false)` y `None` son **neutrales** y no
/// proyectan ninguna condición —proyectar el `false` invertiría el sentido de la
/// regla—.
///
/// La pipeline que la regla resolvió al cargarse, **tomada por referencia**, se
/// clona en cada condición; [`crate::models::CacheRule::evaluate`] la aplica al
/// valor de la petición antes de compararlo con el patrón. Se construye una sola
/// vez al cargar la regla, así que clonarla una vez por condición es
/// irrelevante.
#[must_use]
pub fn project(rule: &Rule, pipeline: &TransformationPipeline) -> Vec<Condition> {
    let mut conditions: Vec<Condition> = Collection::ALL
        .into_iter()
        .filter_map(|collection| {
            let pattern = collection.rule_pattern(rule)?;
            if pattern.is_empty() {
                return None;
            }
            let regex = Regex::new(pattern).ok()?;
            Some(Condition {
                collection,
                key: None,
                pipeline: pipeline.clone(),
                operator: Operator::Rx(regex),
            })
        })
        .collect();

    // La columna `is_tor` se proyecta como una condición booleana más, al final.
    if rule.is_tor == Some(true) {
        conditions.push(Condition {
            collection: Collection::IsTor,
            key: None,
            pipeline: TransformationPipeline::default(),
            operator: Operator::Truthy,
        });
    }

    conditions
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use std::collections::BTreeMap;

    /// Una regla base sin ningún campo restringido ni `is_tor`.
    fn base_rule() -> Rule {
        Rule {
            id: 0,
            name: "test".to_string(),
            description: String::new(),
            weight: 100,
            mode: "enforce".to_string(),
            pipeline: "waf".to_string(),
            transformations: String::new(),
            allow: false,
            is_tor: None,
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
        }
    }

    /// Una petición que trae las 14 colecciones; cada una con su nombre de campo
    /// como valor.
    fn full_request() -> NewRequest {
        NewRequest {
            ip_address: Some("ip_address".to_string()),
            protocol: Some("protocol".to_string()),
            fqdn: Some("fqdn".to_string()),
            path: Some("path".to_string()),
            query: Some("query".to_string()),
            city_name: Some("city_name".to_string()),
            country_name: Some("country_name".to_string()),
            country_code: Some("country_code".to_string()),
            user_agent: Some("user_agent".to_string()),
            method: Some("method".to_string()),
            referer: Some("referer".to_string()),
            content_type: Some("content_type".to_string()),
            accept_language: Some("accept_language".to_string()),
            x_request_id: Some("x_request_id".to_string()),
            cookies: BTreeMap::new(),
            headers: BTreeMap::new(),
            query_args: BTreeMap::new(),
            basename: None,
            extension: None,
            is_tor: None,
            rule_id: None,
            created_at: Utc::now(),
        }
    }

    /// Una petición que no trae ninguna de las 14 colecciones.
    fn empty_request() -> NewRequest {
        NewRequest {
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
            cookies: BTreeMap::new(),
            headers: BTreeMap::new(),
            query_args: BTreeMap::new(),
            basename: None,
            extension: None,
            is_tor: None,
            rule_id: None,
            created_at: Utc::now(),
        }
    }

    /// El patrón fuente del operador, para poder compararlo sin exponer el
    /// `Regex`.
    fn operator_pattern(operator: &Operator) -> &str {
        match operator {
            Operator::Rx(regex) => regex.as_str(),
            Operator::Truthy => "@truthy",
        }
    }

    // ── 1.1 IR ──

    /// infraestructura: verifica la estructura de la condición
    #[test]
    fn test_condition_holds_collection_pipeline_and_operator() {
        let condition = Condition {
            collection: Collection::Path,
            key: None,
            pipeline: TransformationPipeline::default(),
            operator: Operator::Rx(Regex::new(r"^/admin/\d+$").unwrap()),
        };

        assert_eq!(condition.collection, Collection::Path);
        assert!(
            condition.pipeline.is_empty(),
            "la pipeline por defecto es la vacía"
        );
    }

    /// infraestructura: verifica el orden de la cadena
    #[test]
    fn test_collection_all_has_fourteen_in_chain_order() {
        use Collection::UserAgent;
        use Collection::{AcceptLanguage, CityName, ContentType, CountryCode, CountryName};
        use Collection::{Fqdn, IpAddress, Method, Path, Protocol, Query, Referer, RequestId};

        assert_eq!(Collection::ALL.len(), 14);

        assert_eq!(
            Collection::ALL,
            [
                IpAddress,
                Protocol,
                Fqdn,
                Path,
                Query,
                CityName,
                CountryName,
                CountryCode,
                UserAgent,
                Method,
                Referer,
                ContentType,
                AcceptLanguage,
                RequestId,
            ],
            "el orden debe ser el de la cadena `&&` de `evaluate()`"
        );
    }

    /// infraestructura: verifica la clave de la evidencia
    #[test]
    fn test_field_name_is_the_evidence_key() {
        let expected: [(Collection, &str); 14] = [
            (Collection::IpAddress, "ip_address"),
            (Collection::Protocol, "protocol"),
            (Collection::Fqdn, "fqdn"),
            (Collection::Path, "path"),
            (Collection::Query, "query"),
            (Collection::CityName, "city_name"),
            (Collection::CountryName, "country_name"),
            (Collection::CountryCode, "country_code"),
            (Collection::UserAgent, "user_agent"),
            (Collection::Method, "method"),
            (Collection::Referer, "referer"),
            (Collection::ContentType, "content_type"),
            (Collection::AcceptLanguage, "accept_language"),
            (Collection::RequestId, "x_request_id"),
        ];

        for (collection, name) in expected {
            assert_eq!(
                collection.field_name(),
                name,
                "la colección {collection:?} debe exponer el nombre de campo de la evidencia"
            );
        }
    }

    /// infraestructura: verifica el operador Rx
    #[test]
    fn test_operator_rx_decides_with_the_regex() {
        let operator = Operator::Rx(Regex::new(r"^/admin/\d+$").unwrap());

        match &operator {
            Operator::Rx(regex) => {
                assert!(
                    regex.is_match("/admin/42"),
                    "el patrón debe casar `/admin/42`"
                );
                assert!(
                    !regex.is_match("/admin/x"),
                    "el patrón no debe casar `/admin/x`"
                );
            },
            Operator::Truthy => unreachable!("el sujeto de este test es `Operator::Rx`"),
        }
    }

    // ── 1.1b request_value ──

    /// infraestructura: verifica la lectura por colección
    #[test]
    fn test_request_value_reads_each_collection() {
        let full = full_request();
        for collection in Collection::ALL {
            assert_eq!(
                collection.request_value(&full, None).map(String::as_str),
                Some(collection.field_name()),
                "la colección {collection:?} debe leer su campo de la petición"
            );
        }

        let empty = empty_request();
        for collection in Collection::ALL {
            assert_eq!(
                collection.request_value(&empty, None),
                None,
                "la colección {collection:?} debe ser `None` si la petición no la trae"
            );
        }
    }

    // ── 1.2 Proyección ──

    #[test]
    fn test_project_maps_each_restricted_field_to_a_condition() {
        let mut rule = base_rule();
        rule.path = Some("^/admin$".to_string());
        rule.user_agent = Some("curl".to_string());

        let conditions = project(&rule, &TransformationPipeline::default());

        assert_eq!(
            conditions.len(),
            2,
            "dos campos restringidos deben proyectar dos condiciones"
        );
    }

    #[test]
    fn test_project_condition_carries_the_field_pattern() {
        let mut rule = base_rule();
        rule.path = Some("^/admin$".to_string());

        let conditions = project(&rule, &TransformationPipeline::default());

        assert_eq!(
            conditions.len(),
            1,
            "el campo `path` proyecta una condición"
        );
        assert_eq!(conditions[0].collection, Collection::Path);
        assert_eq!(
            operator_pattern(&conditions[0].operator),
            "^/admin$",
            "la condición debe llevar el patrón que la regla tenía en ese campo"
        );
    }

    #[test]
    fn test_project_blank_field_is_not_projected() {
        let mut rule = base_rule();
        rule.path = Some("^/admin$".to_string());
        rule.query = Some(String::new());

        let conditions = project(&rule, &TransformationPipeline::default());

        assert_eq!(
            conditions.len(),
            1,
            "un campo en blanco no proyecta condición"
        );
        assert_eq!(conditions[0].collection, Collection::Path);
    }

    // ── F2b-3: vocabulario `is_tor` y operador `@truthy` ──

    #[test]
    fn test_is_tor_collection_vocabulary() {
        assert_eq!(
            Collection::IsTor.field_name(),
            "is_tor",
            "la colección booleana se persiste con el nombre `is_tor`"
        );
        assert_eq!(
            Collection::from_field_name("is_tor"),
            Some(Collection::IsTor),
            "`from_field_name` debe reconocer `is_tor`"
        );
        assert!(
            !Collection::IsTor.requires_key(),
            "la colección booleana no admite clave"
        );
        assert_eq!(
            Collection::ALL.len(),
            14,
            "`Collection::ALL` sigue siendo las 14 de campo"
        );
        assert!(
            !Collection::ALL.contains(&Collection::IsTor),
            "`is_tor` NO está en `Collection::ALL`"
        );
    }

    #[test]
    fn test_truthy_operator_serializes_as_at_truthy() {
        use crate::models::ConditionInput;

        let condition = Condition {
            collection: Collection::IsTor,
            key: None,
            pipeline: TransformationPipeline::default(),
            operator: Operator::Truthy,
        };

        let input = ConditionInput::from(&condition);

        assert_eq!(
            input.collection, "is_tor",
            "la condición booleana se serializa con su colección"
        );
        assert_eq!(
            input.operator.as_deref(),
            Some("@truthy"),
            "el operador booleano se serializa como `@truthy`"
        );
        assert_eq!(input.pattern, "", "el operador booleano no lleva patrón");
    }

    #[test]
    fn test_truthy_operator_matches_only_when_request_is_tor() {
        use crate::models::CacheRule;

        let condition = Condition {
            collection: Collection::IsTor,
            key: None,
            pipeline: TransformationPipeline::default(),
            operator: Operator::Truthy,
        };
        let cache_rule = CacheRule {
            rule: base_rule(),
            conditions: vec![condition],
        };

        let mut tor = empty_request();
        tor.is_tor = Some(true);
        assert!(
            cache_rule.matches(&tor),
            "`@truthy` cumple cuando el booleano de la petición es exactamente `true`"
        );

        let mut non_tor = empty_request();
        non_tor.is_tor = Some(false);
        assert!(
            !cache_rule.matches(&non_tor),
            "`@truthy` NO cumple con `Some(false)`"
        );

        let unknown = empty_request();
        assert!(
            !cache_rule.matches(&unknown),
            "`@truthy` NO cumple con `None` (ausencia del dato, fail-open)"
        );
    }

    // ── 1.3 Proyección desde la columna `is_tor` ──

    #[test]
    fn test_project_is_tor_true_appends_a_truthy_condition_at_the_end() {
        let mut rule = base_rule();
        rule.path = Some("^/admin$".to_string());
        rule.is_tor = Some(true);

        let conditions = project(&rule, &TransformationPipeline::default());

        assert_eq!(
            conditions.len(),
            2,
            "`is_tor = Some(true)` proyecta una condición de más, al final"
        );
        assert_eq!(conditions[0].collection, Collection::Path);
        assert_eq!(conditions[1].collection, Collection::IsTor);
        assert!(
            matches!(conditions[1].operator, Operator::Truthy),
            "la condición de `is_tor` debe llevar el operador booleano"
        );
        assert!(conditions[1].key.is_none(), "la condición no admite clave");
        assert!(
            conditions[1].pipeline.is_empty(),
            "un booleano no se transforma: la pipeline es vacía"
        );

        let mut tor_only = base_rule();
        tor_only.is_tor = Some(true);
        let conditions = project(&tor_only, &TransformationPipeline::default());
        assert_eq!(
            conditions.len(),
            1,
            "`is_tor` por sí solo proyecta exactamente su condición"
        );
        assert_eq!(conditions[0].collection, Collection::IsTor);
    }

    #[test]
    fn test_project_is_tor_false_projects_nothing() {
        let mut rule = base_rule();
        rule.is_tor = Some(false);
        rule.path = Some("^/admin$".to_string());

        let conditions = project(&rule, &TransformationPipeline::default());

        assert_eq!(
            conditions.len(),
            1,
            "`is_tor = Some(false)` es neutral: solo proyecta `path`"
        );
        assert_eq!(conditions[0].collection, Collection::Path);
        assert!(
            conditions
                .iter()
                .all(|condition| condition.collection != Collection::IsTor),
            "proyectar el `false` invertiría el sentido de la regla: NO debe proyectarse"
        );
    }

    #[test]
    fn test_project_is_tor_none_projects_nothing() {
        let mut rule = base_rule();
        rule.is_tor = None;

        let conditions = project(&rule, &TransformationPipeline::default());

        assert!(
            conditions.is_empty(),
            "`is_tor = None` es neutral y no proyecta ninguna condición"
        );
    }

    /// caracterización: una regla sin campos restringidos proyecta vacío.
    #[test]
    fn test_project_rule_without_restricted_fields_is_empty() {
        let rule = base_rule();

        let conditions = project(&rule, &TransformationPipeline::default());

        assert!(
            conditions.is_empty(),
            "una regla sin campos restringidos proyecta el conjunto vacío"
        );
    }

    /// caracterización: una regex inválida no proyecta condición.
    #[test]
    fn test_project_invalid_regex_is_not_projected() {
        let mut rule = base_rule();
        rule.path = Some("(".to_string());

        let conditions = project(&rule, &TransformationPipeline::default());

        assert!(
            conditions.is_empty(),
            "un patrón que no compila no proyecta condición"
        );
    }

    // Verificación: la proyección es fiel sobre el catálogo completo de plantillas.
    // scenario «La equivalencia se sostiene sobre las plantillas».
    #[test]
    fn test_project_is_faithful_over_the_whole_template_catalog() {
        use crate::templates::{RuleTemplate, all_rule_templates};

        /// Proyecta una plantilla del catálogo a la regla equivalente.
        fn rule_from_template(t: &RuleTemplate) -> Rule {
            let mut rule = base_rule();
            rule.name = t.name.clone();
            rule.ip_address = t.ip_address.clone();
            rule.protocol = t.protocol.clone();
            rule.fqdn = t.fqdn.clone();
            rule.path = t.path.clone();
            rule.query = t.query.clone();
            rule.city_name = t.city_name.clone();
            rule.country_name = t.country_name.clone();
            rule.country_code = t.country_code.clone();
            rule.user_agent = t.user_agent.clone();
            rule.method = t.method.clone();
            rule.referer = t.referer.clone();
            rule.content_type = t.content_type.clone();
            rule.accept_language = t.accept_language.clone();
            rule.x_request_id = t.x_request_id.clone();
            rule.is_tor = t.is_tor;
            rule.transformations = t.transformations.clone();
            rule.pipeline = t.pipeline.clone();
            rule.allow = t.allow;
            rule
        }

        let templates = all_rule_templates();
        assert_eq!(
            templates.len(),
            42,
            "el catálogo son 24 plantillas WAF más 18 de Jail"
        );

        // Este test fija la proyección (estructura) sobre el catálogo; la
        // equivalencia de la *decisión* y de la evidencia la fija
        // `test_evaluate_is_equivalent_over_the_fourteen_fields`, que mapea
        // campo↔colección de forma independiente a `rule_pattern`.
        for template in &templates {
            let rule = rule_from_template(template);

            // Los campos que la plantilla restringe, en el orden de la cadena
            // `&&` de `evaluate()`.
            let mut restricted: Vec<(Collection, String)> = Vec::new();
            for collection in Collection::ALL {
                if let Some(pattern) = collection.rule_pattern(&rule).filter(|p| !p.is_empty()) {
                    assert!(
                        Regex::new(pattern).is_ok(),
                        "la plantilla «{}» restringe {:?} con un patrón que no compila: {:?}",
                        template.name,
                        collection,
                        pattern
                    );
                    restricted.push((collection, pattern.clone()));
                }
            }

            let pipeline =
                TransformationPipeline::parse(&template.transformations).unwrap_or_default();
            let conditions = project(&rule, &pipeline);

            // La proyección esperada es la de los campos restringidos más, tras
            // ellos, la condición `is_tor`/`@truthy` cuando la columna es verdadera.
            let is_tor = usize::from(rule.is_tor == Some(true));
            assert_eq!(
                conditions.len(),
                restricted.len() + is_tor,
                "«{}»: la proyección descartó campos restringidos o `is_tor` (esperado {}, real {})",
                template.name,
                restricted.len() + is_tor,
                conditions.len()
            );

            for (index, (collection, pattern)) in restricted.iter().enumerate() {
                assert_eq!(
                    conditions[index].collection, *collection,
                    "«{}»: la condición {} debe ser de la colección {:?}",
                    template.name, index, collection
                );
                assert_eq!(
                    operator_pattern(&conditions[index].operator),
                    pattern.as_str(),
                    "«{}»: la condición {} debe llevar el patrón de {:?}",
                    template.name,
                    index,
                    collection
                );
            }

            if is_tor == 1 {
                let last = &conditions[restricted.len()];
                assert_eq!(
                    last.collection,
                    Collection::IsTor,
                    "«{}»: la condición de `is_tor` va después de las de los campos",
                    template.name
                );
                assert!(
                    matches!(last.operator, Operator::Truthy),
                    "«{}»: la condición de `is_tor` lleva el operador booleano",
                    template.name
                );
                assert!(
                    last.pipeline.is_empty(),
                    "«{}»: sin transformaciones",
                    template.name
                );
            }
        }

        // La única plantilla con `is_tor` no restringe ningún campo: su única
        // condición proyectada es la de `is_tor`/`@truthy`.
        let tor = templates
            .iter()
            .find(|t| t.name == "Tor exit nodes")
            .expect("el catálogo contiene la plantilla «Tor exit nodes»");
        assert_eq!(tor.is_tor, Some(true));
        assert!(tor.path.is_none() && tor.query.is_none() && tor.user_agent.is_none());
        let tor_conditions = project(&rule_from_template(tor), &TransformationPipeline::default());
        assert_eq!(
            tor_conditions.len(),
            1,
            "«Tor exit nodes» proyecta su única condición `is_tor`"
        );
        assert_eq!(tor_conditions[0].collection, Collection::IsTor);
        assert!(
            matches!(tor_conditions[0].operator, Operator::Truthy),
            "la condición de «Tor exit nodes» lleva `@truthy`"
        );
    }

    #[test]
    fn test_from_field_name_is_the_inverse_of_field_name() {
        // Verificación: cubre el hallazgo de cobertura de la auditoría de F2b-1.
        for collection in Collection::ALL {
            assert_eq!(
                Collection::from_field_name(collection.field_name()),
                Some(collection),
                "`from_field_name` debe invertir `field_name` para {collection:?}"
            );
        }

        // Las colecciones con clave también deben invertirse: sus nombres de
        // campo viven en la misma tabla `rule_conditions` que las 14 históricas.
        let keyed = [
            Collection::RequestCookies,
            Collection::RequestHeaders,
            Collection::ArgsGet,
            Collection::RequestBasename,
            Collection::RequestExtension,
        ];
        for collection in keyed {
            assert_eq!(
                Collection::from_field_name(collection.field_name()),
                Some(collection),
                "`from_field_name` debe invertir `field_name` para {collection:?}"
            );
        }

        assert_eq!(
            Collection::from_field_name("REQUEST_URI_PATH"),
            None,
            "un nombre de campo desconocido debe devolver `None`"
        );
    }

    // ── F2b-2a: vocabulario y matcheo de las colecciones nuevas ──

    #[test]
    fn test_new_collections_have_lowercase_field_names() {
        assert_eq!(Collection::RequestCookies.field_name(), "request_cookies");
        assert_eq!(Collection::RequestHeaders.field_name(), "request_headers");
        assert_eq!(Collection::ArgsGet.field_name(), "args_get");
        assert_eq!(Collection::RequestBasename.field_name(), "request_basename");
        assert_eq!(
            Collection::RequestExtension.field_name(),
            "request_extension"
        );
    }

    #[test]
    fn test_requires_key_is_true_only_for_keyed_collections() {
        assert!(Collection::RequestCookies.requires_key());
        assert!(Collection::RequestHeaders.requires_key());
        assert!(Collection::ArgsGet.requires_key());
        assert!(!Collection::RequestBasename.requires_key());
        assert!(!Collection::RequestExtension.requires_key());
        for c in Collection::ALL {
            assert!(!c.requires_key(), "las 14 actuales no exigen clave");
        }
    }

    /// El regex es el sujeto bajo prueba: se usa `^abc$` de forma deliberada.
    #[allow(clippy::trivial_regex)]
    #[test]
    fn test_cookie_condition_matches_by_name() {
        use crate::models::CacheRule;

        let condition = Condition {
            collection: Collection::RequestCookies,
            key: Some("PHPSESSID".to_string()),
            pipeline: TransformationPipeline::default(),
            operator: Operator::Rx(Regex::new("^abc$").unwrap()),
        };
        let cache_rule = CacheRule {
            rule: base_rule(),
            conditions: vec![condition],
        };
        let mut request = empty_request();
        request
            .cookies
            .insert("PHPSESSID".to_string(), "abc".to_string());

        let outcome = cache_rule.evaluate(&request);
        assert!(
            outcome.matched,
            "la condición de cookie debe casar cuando la petición trae la cookie con la clave"
        );
    }

    /// El regex es el sujeto bajo prueba: se usa `^abc$` de forma deliberada.
    #[allow(clippy::trivial_regex)]
    #[test]
    fn test_missing_cookie_does_not_match() {
        use crate::models::CacheRule;

        let condition = Condition {
            collection: Collection::RequestCookies,
            key: Some("PHPSESSID".to_string()),
            pipeline: TransformationPipeline::default(),
            operator: Operator::Rx(Regex::new("^abc$").unwrap()),
        };
        let cache_rule = CacheRule {
            rule: base_rule(),
            conditions: vec![condition],
        };
        let request = empty_request();

        let outcome = cache_rule.evaluate(&request);
        assert!(
            !outcome.matched,
            "una cookie ausente no debe casar la condición"
        );
    }

    /// El regex es el sujeto bajo prueba: se usan `^bearer x$` y `^secret$` de forma deliberada.
    #[allow(clippy::trivial_regex)]
    #[test]
    fn test_evidence_uses_composite_key() {
        use crate::models::CacheRule;

        let pipeline = TransformationPipeline::parse("lowercase").unwrap_or_default();
        let conditions = vec![
            Condition {
                collection: Collection::RequestHeaders,
                key: Some("authorization".to_string()),
                pipeline: pipeline.clone(),
                operator: Operator::Rx(Regex::new("^bearer x$").unwrap()),
            },
            Condition {
                collection: Collection::RequestHeaders,
                key: Some("x-api-key".to_string()),
                pipeline,
                operator: Operator::Rx(Regex::new("^secret$").unwrap()),
            },
        ];
        let cache_rule = CacheRule {
            rule: base_rule(),
            conditions,
        };
        let mut request = empty_request();
        request
            .headers
            .insert("authorization".to_string(), "Bearer X".to_string());
        request
            .headers
            .insert("x-api-key".to_string(), "SECRET".to_string());

        let outcome = cache_rule.evaluate(&request);
        assert!(outcome.matched, "las dos cabeceras deben casar");
        let evidence = outcome
            .evidence
            .expect("con pipeline no vacía debe haber evidencia");
        assert!(
            evidence.contains_key("request_headers[authorization]"),
            "la evidencia debe usar la clave compuesta para authorization"
        );
        assert!(
            evidence.contains_key("request_headers[x-api-key]"),
            "la evidencia debe usar la clave compuesta para x-api-key"
        );
    }
}
