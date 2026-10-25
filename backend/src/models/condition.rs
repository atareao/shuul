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
/// Las 14 variantes corresponden, una a una, con los 14 campos de [`Rule`] y de
/// [`NewRequest`]. El vocabulario es el de los campos actuales: adoptar el de
/// colecciones de Coraza (`REQUEST_HEADERS`, `ARGS_GET`…) exigiría una
/// extracción que hoy no existe.
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
        }
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
            _ => return None,
        })
    }

    /// El valor de la petición para esta colección, o `None` si no la trae.
    #[must_use]
    pub const fn request_value(self, request: &NewRequest) -> Option<&String> {
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
        }
    }
}

/// Operador de comparación de una condición.
///
/// Nace con una sola variante: declarar las demás antes de implementarlas sería
/// andamiaje muerto.
#[derive(Debug, Clone)]
pub enum Operator {
    /// Compara el valor transformado con un regex.
    Rx(Regex),
}

/// Una condición ya compilada: la colección que restringe, la pipeline que se
/// aplica a su valor y el operador con el que se compara.
#[derive(Debug, Clone)]
pub struct Condition {
    pub collection: Collection,
    pub pipeline: TransformationPipeline,
    pub operator: Operator,
}

/// Proyecta los 14 campos de una regla a su conjunto ordenado de condiciones.
///
/// Recorre [`Collection::ALL`] **en su orden** y proyecta **una** condición por
/// cada campo que la regla restrinja con un patrón **no vacío y compilable**. Un
/// campo sin patrón (`None`), en blanco o cuyo patrón no compile no proyecta
/// nada: su ausencia equivale al brazo `(None, _)` neutral del matcheo.
/// `is_tor` no es una colección y nunca se proyecta.
///
/// La pipeline que la regla resolvió al cargarse, **tomada por referencia**, se
/// clona en cada condición; [`crate::models::CacheRule::evaluate`] la aplica al
/// valor de la petición antes de compararlo con el patrón. Se construye una sola
/// vez al cargar la regla, así que clonarla una vez por condición es
/// irrelevante.
#[must_use]
pub fn project(rule: &Rule, pipeline: &TransformationPipeline) -> Vec<Condition> {
    Collection::ALL
        .into_iter()
        .filter_map(|collection| {
            let pattern = collection.rule_pattern(rule)?;
            if pattern.is_empty() {
                return None;
            }
            let regex = Regex::new(pattern).ok()?;
            Some(Condition {
                collection,
                pipeline: pipeline.clone(),
                operator: Operator::Rx(regex),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

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
        }
    }

    // ── 1.1 IR ──

    /// infraestructura: pasa ya en RED
    #[test]
    fn test_condition_holds_collection_pipeline_and_operator() {
        let condition = Condition {
            collection: Collection::Path,
            pipeline: TransformationPipeline::default(),
            operator: Operator::Rx(Regex::new(r"^/admin/\d+$").unwrap()),
        };

        assert_eq!(condition.collection, Collection::Path);
        assert!(
            condition.pipeline.is_empty(),
            "la pipeline por defecto es la vacía"
        );
    }

    /// infraestructura: pasa ya en RED
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

    /// infraestructura: pasa ya en RED
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

    /// infraestructura: pasa ya en RED
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
        }
    }

    // ── 1.1b request_value ──

    /// infraestructura: pasa ya en RED
    #[test]
    fn test_request_value_reads_each_collection() {
        let full = full_request();
        for collection in Collection::ALL {
            assert_eq!(
                collection.request_value(&full).map(String::as_str),
                Some(collection.field_name()),
                "la colección {collection:?} debe leer su campo de la petición"
            );
        }

        let empty = empty_request();
        for collection in Collection::ALL {
            assert_eq!(
                collection.request_value(&empty),
                None,
                "la colección {collection:?} debe ser `None` si la petición no la trae"
            );
        }
    }

    // ── 1.2 Proyección (RED: falla porque `project` devuelve vacío) ──

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

    #[test]
    fn test_project_is_tor_is_not_projected() {
        let mut rule = base_rule();
        rule.is_tor = Some(true);
        rule.path = Some("^/admin$".to_string());

        let conditions = project(&rule, &TransformationPipeline::default());

        assert_eq!(
            conditions.len(),
            1,
            "`is_tor` no proyecta ninguna condición"
        );
        assert_eq!(conditions[0].collection, Collection::Path);

        let mut tor_only = base_rule();
        tor_only.is_tor = Some(true);
        assert!(
            project(&tor_only, &TransformationPipeline::default()).is_empty(),
            "`is_tor` por sí solo no proyecta condición alguna"
        );
    }

    /// caracterización: pasa ya en RED — el stub ya devuelve el conjunto vacío.
    #[test]
    fn test_project_rule_without_restricted_fields_is_empty() {
        let rule = base_rule();

        let conditions = project(&rule, &TransformationPipeline::default());

        assert!(
            conditions.is_empty(),
            "una regla sin campos restringidos proyecta el conjunto vacío"
        );
    }

    /// caracterización: pasa ya en RED — el stub devuelve vacío y una regex
    /// inválida no debe proyectar condición.
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

    // Verificación: el comportamiento ya existe (F2a GREEN); este test cubre el
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

            assert_eq!(
                conditions.len(),
                restricted.len(),
                "«{}»: la proyección descartó campos restringidos (esperado {}, real {})",
                template.name,
                restricted.len(),
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
        }

        // La única plantilla con `is_tor` no restringe ningún campo: no proyecta
        // ninguna condición.
        let tor = templates
            .iter()
            .find(|t| t.name == "Tor exit nodes")
            .expect("el catálogo contiene la plantilla «Tor exit nodes»");
        assert_eq!(tor.is_tor, Some(true));
        assert!(tor.path.is_none() && tor.query.is_none() && tor.user_agent.is_none());
        assert!(
            project(&rule_from_template(tor), &TransformationPipeline::default()).is_empty(),
            "«Tor exit nodes» no restringe ningún campo y no debe proyectar condición"
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

        assert_eq!(
            Collection::from_field_name("REQUEST_URI_PATH"),
            None,
            "un nombre de campo desconocido debe devolver `None`"
        );
    }
}
