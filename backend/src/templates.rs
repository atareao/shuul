use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct RuleTemplate {
    pub name: String,
    pub description: String,
    pub category: String,
    pub severity: String,
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
    pub is_tor: Option<bool>,
    pub allow: bool,
    pub pipeline: String,
    pub rate_limit_profile_id: Option<i32>,
    pub rate_limit_profile_name: Option<String>,
    pub requires_fqdn: bool,
    pub must_have: bool,
    pub weight: i32,
}

#[derive(Debug, Clone, Serialize)]
pub struct RateLimitProfileTemplate {
    pub id: i32,
    pub name: String,
    pub description: String,
    pub max_requests: i32,
    pub window_seconds: i32,
    pub ban_seconds: i32,
    pub escalation_enabled: bool,
    pub escalation_multipliers: Vec<i32>,
    pub max_ban_seconds: i32,
    pub cooldown_seconds: i32,
    pub fail_codes: Vec<i32>,
}

#[allow(
    clippy::must_use_candidate,
    clippy::return_self_not_must_use,
    clippy::missing_const_for_fn,
    dead_code
)]
impl RuleTemplate {
    #[must_use]
    pub fn waf(name: &str, description: &str) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            category: "general".into(),
            severity: "medio".into(),
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
            allow: false,
            pipeline: "waf".into(),
            rate_limit_profile_id: None,
            rate_limit_profile_name: None,
            requires_fqdn: false,
            must_have: false,
            weight: 100,
        }
    }

    #[must_use]
    pub fn jail(name: &str, description: &str) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            category: "general".into(),
            severity: "medio".into(),
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
            allow: true,
            pipeline: "jail".into(),
            rate_limit_profile_id: None,
            rate_limit_profile_name: None,
            requires_fqdn: false,
            must_have: false,
            weight: 100,
        }
    }

    pub fn category(mut self, v: &str) -> Self {
        self.category = v.into();
        self
    }
    pub fn severity(mut self, v: &str) -> Self {
        self.severity = v.into();
        self
    }
    pub fn ip_address(mut self, v: &str) -> Self {
        self.ip_address = Some(v.into());
        self
    }
    pub fn protocol(mut self, v: &str) -> Self {
        self.protocol = Some(v.into());
        self
    }
    pub fn fqdn(mut self, v: &str) -> Self {
        self.fqdn = Some(v.into());
        self
    }
    pub fn path(mut self, v: &str) -> Self {
        self.path = Some(v.into());
        self
    }
    pub fn query(mut self, v: &str) -> Self {
        self.query = Some(v.into());
        self
    }
    pub fn city_name(mut self, v: &str) -> Self {
        self.city_name = Some(v.into());
        self
    }
    pub fn country_name(mut self, v: &str) -> Self {
        self.country_name = Some(v.into());
        self
    }
    pub fn country_code(mut self, v: &str) -> Self {
        self.country_code = Some(v.into());
        self
    }
    pub fn user_agent(mut self, v: &str) -> Self {
        self.user_agent = Some(v.into());
        self
    }
    pub fn method(mut self, v: &str) -> Self {
        self.method = Some(v.into());
        self
    }
    pub fn referer(mut self, v: &str) -> Self {
        self.referer = Some(v.into());
        self
    }
    pub fn content_type(mut self, v: &str) -> Self {
        self.content_type = Some(v.into());
        self
    }
    pub fn accept_language(mut self, v: &str) -> Self {
        self.accept_language = Some(v.into());
        self
    }
    pub fn x_request_id(mut self, v: &str) -> Self {
        self.x_request_id = Some(v.into());
        self
    }
    #[allow(clippy::wrong_self_convention)]
    pub fn is_tor(mut self, v: bool) -> Self {
        self.is_tor = Some(v);
        self
    }
    pub fn allow(mut self, v: bool) -> Self {
        self.allow = v;
        self
    }
    pub fn requires_fqdn(mut self, v: bool) -> Self {
        self.requires_fqdn = v;
        self
    }
    pub fn must_have(mut self, v: bool) -> Self {
        self.must_have = v;
        self
    }
    pub fn weight(mut self, v: i32) -> Self {
        self.weight = v;
        self
    }
    pub fn rate_limit_profile(mut self, id: i32, name: &str) -> Self {
        self.rate_limit_profile_id = Some(id);
        self.rate_limit_profile_name = Some(name.into());
        self
    }
}

#[allow(clippy::too_many_lines)]
#[must_use]
pub fn all_rule_templates() -> Vec<RuleTemplate> {
    vec![
        // ===== WAF TEMPLATES - Must-have (12) =====
        // 1
        RuleTemplate { name: "SQL injection probes".into(), description: "Detecta patrones comunes de SQL injection en query params".into(), category: "seguridad".into(), severity: "alto".into(), path: None, query: Some(r"(?i)(union\s+select|select\s+from|insert\s+into|drop\s+table|delete\s+from|alter\s+table|create\s+table|\bexec\b\s*sp_|\bexec\b\s*xp_|pg_sleep|waitfor\s+delay|benchmark\(|sleep\s*\(|--|#|\bOR\b\s+\d+=\d+|\bAND\b\s+\d+=\d+)".into()), user_agent: None, method: None, ip_address: None, protocol: None, fqdn: None, city_name: None, country_name: None, country_code: None, referer: None, content_type: None, accept_language: None, x_request_id: None, is_tor: None, allow: false, pipeline: "waf".into(), rate_limit_profile_id: None, rate_limit_profile_name: None, requires_fqdn: false, must_have: true, weight: 100 },
        // 2
        RuleTemplate { name: "XSS probes".into(), description: "Detecta patrones comunes de Cross-Site Scripting en query params".into(), category: "seguridad".into(), severity: "medio".into(), path: None, query: Some(r"(?i)(<script|<img\s+.*onerror|<svg\s+.*onload|onerror=|onload=|onclick=|onmouseover=|javascript:\s*$|alert\(|prompt\(|confirm\(|document\.cookie|document\.location|fromCharCode|eval\(|String\.fromCharCode|<iframe|<embed|<object)".into()), user_agent: None, method: None, ip_address: None, protocol: None, fqdn: None, city_name: None, country_name: None, country_code: None, referer: None, content_type: None, accept_language: None, x_request_id: None, is_tor: None, allow: false, pipeline: "waf".into(), rate_limit_profile_id: None, rate_limit_profile_name: None, requires_fqdn: false, must_have: true, weight: 200 },
        // 3
        RuleTemplate { name: "Command injection probes".into(), description: "Detecta intentos de inyeccion de comandos en query params".into(), category: "probes".into(), severity: "alto".into(), path: None, query: Some(r"(?i)([|;&]\s*(cat|wget|curl|bash|sh|python|perl|ruby|nc|nmap|which|whoami|id|uname|ls\s+-la|rm\s+-rf|chmod|chown|sudo|passwd|/etc/passwd|/etc/shadow)|\$\(|`.*`|%0a|%0d|%00)".into()), user_agent: None, method: None, ip_address: None, protocol: None, fqdn: None, city_name: None, country_name: None, country_code: None, referer: None, content_type: None, accept_language: None, x_request_id: None, is_tor: None, allow: false, pipeline: "waf".into(), rate_limit_profile_id: None, rate_limit_profile_name: None, requires_fqdn: false, must_have: true, weight: 100 },
        // 4
        RuleTemplate { name: "Log4j / Log4Shell".into(), description: "Detecta intentos de explotacion de Log4j (CVE-2021-44228)".into(), category: "probes".into(), severity: "critico".into(), path: None, query: None, user_agent: Some(r"(?i)(\$\{jndi:|\$\{log4j:|\$\{lower:|\$\{env:|\$\{sys:|\$\{base64:|\$\{::-)".into()), method: None, ip_address: None, protocol: None, fqdn: None, city_name: None, country_name: None, country_code: None, referer: None, content_type: None, accept_language: None, x_request_id: None, is_tor: None, allow: false, pipeline: "waf".into(), rate_limit_profile_id: None, rate_limit_profile_name: None, requires_fqdn: false, must_have: true, weight: 10 },
        // 5
        RuleTemplate { name: "Path traversal probes".into(), description: "Detecta intentos de path traversal (../)".into(), category: "seguridad".into(), severity: "alto".into(), path: Some(r"\.\./".into()), query: None, user_agent: None, method: None, ip_address: None, protocol: None, fqdn: None, city_name: None, country_name: None, country_code: None, referer: None, content_type: None, accept_language: None, x_request_id: None, is_tor: None, allow: false, pipeline: "waf".into(), rate_limit_profile_id: None, rate_limit_profile_name: None, requires_fqdn: false, must_have: true, weight: 100 },
        // 6 - Archivos sensibles (merged: .env, .bak, .sql, .config, .yml, .json, .ini, .pem, .key, .DS_Store, composer.json, etc.)
        RuleTemplate { name: "Archivos sensibles".into(), description: "Bloquea acceso a archivos de configuracion y backups".into(), category: "seguridad".into(), severity: "alto".into(), path: Some(r"\.(env|bak|sql|dump|config|yml|yaml|json|log|ini|pem|key|ppk|asc|gpg|DS_Store|swp|swo)$|^/(composer\.json|package\.json|yarn\.lock|package-lock\.json)".into()), query: None, user_agent: None, method: None, ip_address: None, protocol: None, fqdn: None, city_name: None, country_name: None, country_code: None, referer: None, content_type: None, accept_language: None, x_request_id: None, is_tor: None, allow: false, pipeline: "waf".into(), rate_limit_profile_id: None, rate_limit_profile_name: None, requires_fqdn: false, must_have: true, weight: 100 },
        // 7 - Directorios de sistema
        RuleTemplate { name: "Directorios de sistema".into(), description: "Bloquea acceso a directorios internos del proyecto".into(), category: "seguridad".into(), severity: "critico".into(), path: Some(r"^/(vendor|node_modules|storage|cache|logs|tmp|\.git|\.env)".into()), query: None, user_agent: None, method: None, ip_address: None, protocol: None, fqdn: None, city_name: None, country_name: None, country_code: None, referer: None, content_type: None, accept_language: None, x_request_id: None, is_tor: None, allow: false, pipeline: "waf".into(), rate_limit_profile_id: None, rate_limit_profile_name: None, requires_fqdn: false, must_have: true, weight: 10 },
        // 8 - Docker socket exposure
        RuleTemplate { name: "Docker socket exposure".into(), description: "Bloquea intentos de acceso al socket de Docker".into(), category: "probes".into(), severity: "critico".into(), path: Some(r"/(docker\.sock|var/run/docker\.sock)".into()), query: None, user_agent: None, method: None, ip_address: None, protocol: None, fqdn: None, city_name: None, country_name: None, country_code: None, referer: None, content_type: None, accept_language: None, x_request_id: None, is_tor: None, allow: false, pipeline: "waf".into(), rate_limit_profile_id: None, rate_limit_profile_name: None, requires_fqdn: false, must_have: true, weight: 10 },
        // 9 - phpinfo exposure
        RuleTemplate { name: "phpinfo exposure".into(), description: "Bloquea phpinfo() que expone configuracion del servidor".into(), category: "seguridad".into(), severity: "alto".into(), path: Some(r"(phpinfo|info\.php)".into()), query: None, user_agent: None, method: None, ip_address: None, protocol: None, fqdn: None, city_name: None, country_name: None, country_code: None, referer: None, content_type: None, accept_language: None, x_request_id: None, is_tor: None, allow: false, pipeline: "waf".into(), rate_limit_profile_id: None, rate_limit_profile_name: None, requires_fqdn: false, must_have: true, weight: 100 },
        // 10 - User-Agent vacio
        RuleTemplate { name: "User-Agent vacio".into(), description: "Bloquea peticiones sin User-Agent (bots/scripts)".into(), category: "bots".into(), severity: "medio".into(), path: None, query: None, user_agent: Some(r"^$".into()), method: None, ip_address: None, protocol: None, fqdn: None, city_name: None, country_name: None, country_code: None, referer: None, content_type: None, accept_language: None, x_request_id: None, is_tor: None, allow: false, pipeline: "waf".into(), rate_limit_profile_id: None, rate_limit_profile_name: None, requires_fqdn: false, must_have: true, weight: 200 },
        // 11 - User-Agent bots conocidos
        RuleTemplate { name: "User-Agent bots conocidos".into(), description: "Bloquea user-agents de bots maliciosos conocidos".into(), category: "bots".into(), severity: "medio".into(), path: None, query: None, user_agent: Some(r"(?i)(curl|wget|python-requests|go-http-client|Java/|libwww|phpcrawl|nikto|nessus|nmap|openvas|masscan|zgrab|sqlmap|havij|wpscan|acunetix|burp|hydra|metasploit|gobuster|dirbuster|wfuzz|WhatWeb|recon-ng|theharvester)".into()), method: None, ip_address: None, protocol: None, fqdn: None, city_name: None, country_name: None, country_code: None, referer: None, content_type: None, accept_language: None, x_request_id: None, is_tor: None, allow: false, pipeline: "waf".into(), rate_limit_profile_id: None, rate_limit_profile_name: None, requires_fqdn: false, must_have: true, weight: 200 },
        // 12 - Apache/Nginx status
        RuleTemplate { name: "Apache/Nginx status".into(), description: "Bloquea paginas de estado de servidores web".into(), category: "servidores".into(), severity: "alto".into(), path: Some(r"^/(server-status|server-info|nginx_status|fpm-status|fpm-ping)".into()), query: None, user_agent: None, method: None, ip_address: None, protocol: None, fqdn: None, city_name: None, country_name: None, country_code: None, referer: None, content_type: None, accept_language: None, x_request_id: None, is_tor: None, allow: false, pipeline: "waf".into(), rate_limit_profile_id: None, rate_limit_profile_name: None, requires_fqdn: false, must_have: true, weight: 100 },
        // 13 - PHPUnit probe (RCE) - CVE-2017-9841
        RuleTemplate { name: "PHPUnit probe (RCE)".into(), description: "Detecta intentos de explotacion de PHPUnit (CVE-2017-9841)".into(), category: "probes".into(), severity: "critico".into(), path: Some(r"^/vendor/phpunit".into()), query: None, user_agent: None, method: None, ip_address: None, protocol: None, fqdn: None, city_name: None, country_name: None, country_code: None, referer: None, content_type: None, accept_language: None, x_request_id: None, is_tor: None, allow: false, pipeline: "waf".into(), rate_limit_profile_id: None, rate_limit_profile_name: None, requires_fqdn: false, must_have: true, weight: 10 },

        // ===== WAF TEMPLATES - Specific (9) =====
        // 13 - WordPress uploads
        RuleTemplate { name: "WordPress uploads".into(), description: "Bloquea ejecucion de scripts en directorio de uploads".into(), category: "wordpress".into(), severity: "alto".into(), path: Some(r"^/wp-content/uploads/.*\.(php|phtml|php5)$".into()), query: None, user_agent: None, method: None, ip_address: None, protocol: None, fqdn: None, city_name: None, country_name: None, country_code: None, referer: None, content_type: None, accept_language: None, x_request_id: None, is_tor: None, allow: false, pipeline: "waf".into(), rate_limit_profile_id: None, rate_limit_profile_name: None, requires_fqdn: true, must_have: false, weight: 100 },
        // 14 - Adminer + phpMyAdmin
        RuleTemplate { name: "Adminer + phpMyAdmin".into(), description: "Bloquea herramientas de gestion de bases de datos".into(), category: "paneles".into(), severity: "critico".into(), path: Some(r"^/(adminer|adminer-|phpmyadmin[0-9]*|pma[0-9]*|mysql[0-9]*|phpPgAdmin|sqldb|sqladmin)".into()), query: None, user_agent: None, method: None, ip_address: None, protocol: None, fqdn: None, city_name: None, country_name: None, country_code: None, referer: None, content_type: None, accept_language: None, x_request_id: None, is_tor: None, allow: false, pipeline: "waf".into(), rate_limit_profile_id: None, rate_limit_profile_name: None, requires_fqdn: true, must_have: false, weight: 50 },
        // 15 - Laravel
        RuleTemplate { name: "Laravel".into(), description: "Bloquea herramientas de depuracion de Laravel en produccion".into(), category: "laravel".into(), severity: "alto".into(), path: Some(r"^/(_debugbar|_ignition|telescope)".into()), query: None, user_agent: None, method: None, ip_address: None, protocol: None, fqdn: None, city_name: None, country_name: None, country_code: None, referer: None, content_type: None, accept_language: None, x_request_id: None, is_tor: None, allow: false, pipeline: "waf".into(), rate_limit_profile_id: None, rate_limit_profile_name: None, requires_fqdn: true, must_have: false, weight: 100 },
        // 16 - Swagger / API docs
        RuleTemplate { name: "Swagger / API docs".into(), description: "Bloquea documentacion interactiva de API en produccion".into(), category: "api".into(), severity: "medio".into(), path: Some(r"^/(swagger|api/docs|openapi|redoc|api/swagger)".into()), query: None, user_agent: None, method: None, ip_address: None, protocol: None, fqdn: None, city_name: None, country_name: None, country_code: None, referer: None, content_type: None, accept_language: None, x_request_id: None, is_tor: None, allow: false, pipeline: "waf".into(), rate_limit_profile_id: None, rate_limit_profile_name: None, requires_fqdn: true, must_have: false, weight: 200 },
        // 17 - API admin endpoints
        RuleTemplate { name: "API admin endpoints".into(), description: "Bloquea endpoints administrativos de API".into(), category: "api".into(), severity: "alto".into(), path: Some(r"^/(api/v1/admin|admin/api)".into()), query: None, user_agent: None, method: None, ip_address: None, protocol: None, fqdn: None, city_name: None, country_name: None, country_code: None, referer: None, content_type: None, accept_language: None, x_request_id: None, is_tor: None, allow: false, pipeline: "waf".into(), rate_limit_profile_id: None, rate_limit_profile_name: None, requires_fqdn: true, must_have: false, weight: 100 },
        // 18 - Drupal install
        RuleTemplate { name: "Drupal install".into(), description: "Bloquea el instalador de Drupal (debe eliminarse tras instalar)".into(), category: "drupal".into(), severity: "critico".into(), path: Some(r"^/install\.php".into()), query: None, user_agent: None, method: None, ip_address: None, protocol: None, fqdn: None, city_name: None, country_name: None, country_code: None, referer: None, content_type: None, accept_language: None, x_request_id: None, is_tor: None, allow: false, pipeline: "waf".into(), rate_limit_profile_id: None, rate_limit_profile_name: None, requires_fqdn: true, must_have: false, weight: 50 },
        // 19 - Symfony
        RuleTemplate { name: "Symfony".into(), description: "Bloquea el profiler y Web Debug Toolbar de Symfony en produccion".into(), category: "seguridad".into(), severity: "alto".into(), path: Some(r"^/(_profiler|_wdt)".into()), query: None, user_agent: None, method: None, ip_address: None, protocol: None, fqdn: None, city_name: None, country_name: None, country_code: None, referer: None, content_type: None, accept_language: None, x_request_id: None, is_tor: None, allow: false, pipeline: "waf".into(), rate_limit_profile_id: None, rate_limit_profile_name: None, requires_fqdn: true, must_have: false, weight: 100 },
        // 20 - PrestaShop install
        RuleTemplate { name: "PrestaShop install".into(), description: "Bloquea el instalador de PrestaShop (debe eliminarse tras instalar)".into(), category: "cms".into(), severity: "critico".into(), path: Some(r"^/install/".into()), query: None, user_agent: None, method: None, ip_address: None, protocol: None, fqdn: None, city_name: None, country_name: None, country_code: None, referer: None, content_type: None, accept_language: None, x_request_id: None, is_tor: None, allow: false, pipeline: "waf".into(), rate_limit_profile_id: None, rate_limit_profile_name: None, requires_fqdn: true, must_have: false, weight: 50 },
        // 21 - Geo blocking
        RuleTemplate { name: "Geo blocking".into(), description: "Deniega trafico de paises donde no operas".into(), category: "geo".into(), severity: "bajo".into(), path: None, query: None, user_agent: None, method: None, ip_address: None, protocol: None, fqdn: None, city_name: None, country_name: None, country_code: Some(r"^(RU|CN|KP|IR|BY|AF|SY|VE|CU|SD|NL|DE|SG|HK|UA)$".into()), referer: None, content_type: None, accept_language: None, x_request_id: None, is_tor: None, allow: false, pipeline: "waf".into(), rate_limit_profile_id: None, rate_limit_profile_name: None, requires_fqdn: false, must_have: false, weight: 300 },
        // 22 - Tor exit nodes
        RuleTemplate { name: "Tor exit nodes".into(), description: "Bloquea trafico proveniente de nodos de salida de la red Tor".into(), category: "tor".into(), severity: "alto".into(), path: None, query: None, user_agent: None, method: None, ip_address: None, protocol: None, fqdn: None, city_name: None, country_name: None, country_code: None, referer: None, content_type: None, accept_language: None, x_request_id: None, is_tor: Some(true), allow: false, pipeline: "waf".into(), rate_limit_profile_id: None, rate_limit_profile_name: None, requires_fqdn: false, must_have: false, weight: 300 },

        // ===== JAIL TEMPLATES - Scanners/Catch-all (4) =====
        // 22 - Scanner generico 404
        RuleTemplate { name: "Scanner generico 404".into(), description: "Rate limiting global basado en 404: cualquier IP que genere demasiados errores 404 sera limitada".into(), category: "probes".into(), severity: "medio".into(), path: None, query: None, user_agent: None, method: None, ip_address: None, protocol: None, fqdn: None, city_name: None, country_name: None, country_code: None, referer: None, content_type: None, accept_language: None, x_request_id: None, is_tor: None, allow: true, pipeline: "jail".into(), rate_limit_profile_id: Some(3), rate_limit_profile_name: Some("Path Scanning".into()), requires_fqdn: false, must_have: true, weight: 9998 },
        // 23 - Scanner vulnerabilidades web
        RuleTemplate { name: "Scanner vulnerabilidades web".into(), description: "Rate limiting agresivo para escaneres que buscan shells, backdoors y exploits".into(), category: "probes".into(), severity: "critico".into(), path: Some(r"(shell|cmd|rce|webshell|backdoor|hack|exploit|cve-\d{4}|payload|eval|exec|server-status|server-info|info\.php|phpinfo)".into()), query: None, user_agent: None, method: None, ip_address: None, protocol: None, fqdn: None, city_name: None, country_name: None, country_code: None, referer: None, content_type: None, accept_language: None, x_request_id: None, is_tor: None, allow: true, pipeline: "jail".into(), rate_limit_profile_id: Some(9), rate_limit_profile_name: Some("Scanner Aggressive".into()), requires_fqdn: false, must_have: true, weight: 10 },
        // 24 - Scanner directorios+configs+paneles
        RuleTemplate { name: "Scanner directorios+configs+paneles".into(), description: "Rate limiting para escaneres que prueban directorios, configs y paneles comunes".into(), category: "probes".into(), severity: "alto".into(), path: Some(r"^/(admin|administrator|backup|backups|tmp|temp|test|tests|dev|development|private|restrito|old|new)(/|$)|\.(env|bak|sql|dump|config|yml|yaml|json|log|ini)$|/(wp-config|configuration\.php|settings|database\.yml)|^/(panel|cpanel|plesk|webmin|vestacp|kloxo|directadmin|phpmyadmin|pma|myadmin|mysql|phpPgAdmin|pgadmin)".into()), query: None, user_agent: None, method: None, ip_address: None, protocol: None, fqdn: None, city_name: None, country_name: None, country_code: None, referer: None, content_type: None, accept_language: None, x_request_id: None, is_tor: None, allow: true, pipeline: "jail".into(), rate_limit_profile_id: Some(3), rate_limit_profile_name: Some("Path Scanning".into()), requires_fqdn: false, must_have: true, weight: 10 },
        // 25 - Global Shield catch-all
        RuleTemplate { name: "Global Shield catch-all".into(), description: "Rate limiting global: cualquier IP que genere demasiados errores en 60s sera rate-limitada".into(), category: "bots".into(), severity: "medio".into(), path: None, query: None, user_agent: None, method: None, ip_address: None, protocol: None, fqdn: None, city_name: None, country_name: None, country_code: None, referer: None, content_type: None, accept_language: None, x_request_id: None, is_tor: None, allow: true, pipeline: "jail".into(), rate_limit_profile_id: Some(8), rate_limit_profile_name: Some("Global Shield".into()), requires_fqdn: false, must_have: false, weight: 9999 },

        // ===== JAIL TEMPLATES - Service protection (8) =====
        // 26 - Login endpoints
        RuleTemplate { name: "Login endpoints".into(), description: "Protege endpoints de autenticacion contra fuerza bruta".into(), category: "bots".into(), severity: "critico".into(), path: Some(r"^/(login|signin|auth|user|wp-login|wp-login\.php|administrator)".into()), query: None, user_agent: None, method: None, ip_address: None, protocol: None, fqdn: None, city_name: None, country_name: None, country_code: None, referer: None, content_type: None, accept_language: None, x_request_id: None, is_tor: None, allow: true, pipeline: "jail".into(), rate_limit_profile_id: Some(1), rate_limit_profile_name: Some("Auth Brute Force".into()), requires_fqdn: false, must_have: false, weight: 50 },
        // 27 - Admin panels
        RuleTemplate { name: "Admin panels".into(), description: "Protege paneles de administracion contra accesos no autorizados".into(), category: "paneles".into(), severity: "alto".into(), path: Some(r"^/(admin|dashboard|wp-admin|manager)".into()), query: None, user_agent: None, method: None, ip_address: None, protocol: None, fqdn: None, city_name: None, country_name: None, country_code: None, referer: None, content_type: None, accept_language: None, x_request_id: None, is_tor: None, allow: true, pipeline: "jail".into(), rate_limit_profile_id: Some(1), rate_limit_profile_name: Some("Auth Brute Force".into()), requires_fqdn: false, must_have: false, weight: 100 },
        // 28 - xmlrpc (WAF, allow=false)
        RuleTemplate { name: "xmlrpc".into(), description: "Bloquea xmlrpc.php vector clasico de fuerza bruta y DDoS".into(), category: "probes".into(), severity: "critico".into(), path: Some(r"^/xmlrpc\.php".into()), query: None, user_agent: None, method: None, ip_address: None, protocol: None, fqdn: None, city_name: None, country_name: None, country_code: None, referer: None, content_type: None, accept_language: None, x_request_id: None, is_tor: None, allow: false, pipeline: "waf".into(), rate_limit_profile_id: None, rate_limit_profile_name: None, requires_fqdn: false, must_have: false, weight: 50 },
        // 29 - Paneles de gestion
        RuleTemplate { name: "Paneles de gestion".into(), description: "Rate limiting en paneles de administracion y gestion".into(), category: "paneles".into(), severity: "critico".into(), path: Some(r"^/(phpmyadmin|pma|mysql|phpPgAdmin|cpanel|whm|webmin|usermin|jenkins|ci|portainer|pgadmin|pgadmin4)".into()), query: None, user_agent: None, method: None, ip_address: None, protocol: None, fqdn: None, city_name: None, country_name: None, country_code: None, referer: None, content_type: None, accept_language: None, x_request_id: None, is_tor: None, allow: true, pipeline: "jail".into(), rate_limit_profile_id: Some(2), rate_limit_profile_name: Some("Admin Guard".into()), requires_fqdn: false, must_have: false, weight: 50 },
        // 30 - Auth endpoints sensibles
        RuleTemplate { name: "Auth endpoints sensibles".into(), description: "Rate limiting en endpoints de autenticacion sensibles".into(), category: "api".into(), severity: "alto".into(), path: Some(r"^/(api|auth)/(register|signup|reset|forgot|recover|callback|oauth)".into()), query: None, user_agent: None, method: None, ip_address: None, protocol: None, fqdn: None, city_name: None, country_name: None, country_code: None, referer: None, content_type: None, accept_language: None, x_request_id: None, is_tor: None, allow: true, pipeline: "jail".into(), rate_limit_profile_id: Some(2), rate_limit_profile_name: Some("Admin Guard".into()), requires_fqdn: false, must_have: false, weight: 100 },
        // 31 - API REST
        RuleTemplate { name: "API REST".into(), description: "Rate limiting para endpoints de API REST".into(), category: "api".into(), severity: "medio".into(), path: Some(r"^/(api/|wp-json/|rest/)".into()), query: None, user_agent: None, method: None, ip_address: None, protocol: None, fqdn: None, city_name: None, country_name: None, country_code: None, referer: None, content_type: None, accept_language: None, x_request_id: None, is_tor: None, allow: true, pipeline: "jail".into(), rate_limit_profile_id: Some(4), rate_limit_profile_name: Some("API Abuse".into()), requires_fqdn: false, must_have: false, weight: 200 },
        // 32 - GraphQL
        RuleTemplate { name: "GraphQL".into(), description: "Rate limiting para endpoints GraphQL contra abusos".into(), category: "api".into(), severity: "medio".into(), path: Some(r"^/(graphql|api/graphql)".into()), query: None, user_agent: None, method: None, ip_address: None, protocol: None, fqdn: None, city_name: None, country_name: None, country_code: None, referer: None, content_type: None, accept_language: None, x_request_id: None, is_tor: None, allow: true, pipeline: "jail".into(), rate_limit_profile_id: Some(4), rate_limit_profile_name: Some("API Abuse".into()), requires_fqdn: false, must_have: false, weight: 200 },
        // 33 - Search endpoints
        RuleTemplate { name: "Search endpoints".into(), description: "Rate limiting en endpoints de busqueda para prevenir abusos".into(), category: "api".into(), severity: "medio".into(), path: Some(r"^/(search|api/search|s/|api/s)".into()), query: None, user_agent: None, method: None, ip_address: None, protocol: None, fqdn: None, city_name: None, country_name: None, country_code: None, referer: None, content_type: None, accept_language: None, x_request_id: None, is_tor: None, allow: true, pipeline: "jail".into(), rate_limit_profile_id: Some(4), rate_limit_profile_name: Some("API Abuse".into()), requires_fqdn: false, must_have: false, weight: 200 },

        // ===== JAIL TEMPLATES - Specific scenarios (6) =====
        // 34 - File upload abuse
        RuleTemplate { name: "File upload abuse".into(), description: "Rate limiting en endpoints de subida de archivos".into(), category: "seguridad".into(), severity: "alto".into(), path: Some(r"^/(upload|file-upload|api/upload|wp-content/uploads)".into()), query: None, user_agent: None, method: None, ip_address: None, protocol: None, fqdn: None, city_name: None, country_name: None, country_code: None, referer: None, content_type: None, accept_language: None, x_request_id: None, is_tor: None, allow: true, pipeline: "jail".into(), rate_limit_profile_id: Some(2), rate_limit_profile_name: Some("Admin Guard".into()), requires_fqdn: false, must_have: false, weight: 100 },
        // 35 - Comment / form spam
        RuleTemplate { name: "Comment / form spam".into(), description: "Rate limiting en formularios de comentarios contra spam".into(), category: "bots".into(), severity: "medio".into(), path: Some(r"^/(wp-comments-post|comment|comments/ajax)".into()), query: None, user_agent: None, method: Some(r"(?i)^POST$".into()), ip_address: None, protocol: None, fqdn: None, city_name: None, country_name: None, country_code: None, referer: None, content_type: None, accept_language: None, x_request_id: None, is_tor: None, allow: true, pipeline: "jail".into(), rate_limit_profile_id: Some(5), rate_limit_profile_name: Some("Scraping".into()), requires_fqdn: false, must_have: false, weight: 200 },
        // 36 - Webmail
        RuleTemplate { name: "Webmail".into(), description: "Rate limiting en logins de webmail contra fuerza bruta".into(), category: "webmail".into(), severity: "critico".into(), path: Some(r"^/(roundcube|webmail|mail|rainloop|snappymail)/".into()), query: None, user_agent: None, method: None, ip_address: None, protocol: None, fqdn: None, city_name: None, country_name: None, country_code: None, referer: None, content_type: None, accept_language: None, x_request_id: None, is_tor: None, allow: true, pipeline: "jail".into(), rate_limit_profile_id: Some(1), rate_limit_profile_name: Some("Auth Brute Force".into()), requires_fqdn: false, must_have: false, weight: 50 },
        // 37 - Scraping por pais
        RuleTemplate { name: "Scraping por pais".into(), description: "Rate limiting por pais para prevenir scraping".into(), category: "bots".into(), severity: "medio".into(), path: None, query: None, user_agent: None, method: None, ip_address: None, protocol: None, fqdn: None, city_name: None, country_name: None, country_code: Some(r"^(RU|CN|NL|DE|SG|HK)$".into()), referer: None, content_type: None, accept_language: None, x_request_id: None, is_tor: None, allow: true, pipeline: "jail".into(), rate_limit_profile_id: Some(5), rate_limit_profile_name: Some("Scraping".into()), requires_fqdn: false, must_have: false, weight: 200 },
        // 38 - Health & Webhooks
        RuleTemplate { name: "Health & Webhooks".into(), description: "Rate limiting para endpoints de salud y webhooks".into(), category: "api".into(), severity: "bajo".into(), path: Some(r"^/(health|healthz|ready|webhook|api/webhook)".into()), query: None, user_agent: None, method: None, ip_address: None, protocol: None, fqdn: None, city_name: None, country_name: None, country_code: None, referer: None, content_type: None, accept_language: None, x_request_id: None, is_tor: None, allow: true, pipeline: "jail".into(), rate_limit_profile_id: Some(6), rate_limit_profile_name: Some("Health & Webhooks".into()), requires_fqdn: false, must_have: false, weight: 100 },
        // 39 - Recidive
        RuleTemplate { name: "Recidive".into(), description: "Rate limiting para IPs reincidentes (3 reincidencias en 48h -> 7 dias)".into(), category: "bots".into(), severity: "medio".into(), path: None, query: None, user_agent: None, method: None, ip_address: None, protocol: None, fqdn: None, city_name: None, country_name: None, country_code: None, referer: None, content_type: None, accept_language: None, x_request_id: None, is_tor: None, allow: true, pipeline: "jail".into(), rate_limit_profile_id: Some(7), rate_limit_profile_name: Some("Recidive".into()), requires_fqdn: false, must_have: false, weight: 9997 },
        // 40 - API Brute Force (general)
        RuleTemplate { name: "API Brute Force general".into(), description: "Rate limiting general para endpoints de API contra fuerza bruta".into(), category: "api".into(), severity: "alto".into(), path: Some(r"^/(api/v1|v1)/".into()), query: None, user_agent: None, method: None, ip_address: None, protocol: None, fqdn: None, city_name: None, country_name: None, country_code: None, referer: None, content_type: None, accept_language: None, x_request_id: None, is_tor: None, allow: true, pipeline: "jail".into(), rate_limit_profile_id: Some(1), rate_limit_profile_name: Some("Auth Brute Force".into()), requires_fqdn: false, must_have: false, weight: 200 },
    ]
}

#[allow(clippy::too_many_lines)]
#[must_use]
pub fn all_rate_limit_profile_templates() -> Vec<RateLimitProfileTemplate> {
    vec![
        RateLimitProfileTemplate {
            id: 1,
            name: "Auth Brute Force".into(),
            description: "5 requests in 5 minutes → 15min ban with escalation".into(),
            max_requests: 5,
            window_seconds: 300,
            ban_seconds: 900,
            escalation_enabled: true,
            escalation_multipliers: vec![1, 2, 4, 8],
            max_ban_seconds: 604_800,
            cooldown_seconds: 30,
            fail_codes: vec![401],
        },
        RateLimitProfileTemplate {
            id: 2,
            name: "Admin Guard".into(),
            description: "5 requests in 5 minutes → 1h ban with escalation".into(),
            max_requests: 5,
            window_seconds: 300,
            ban_seconds: 3600,
            escalation_enabled: true,
            escalation_multipliers: vec![1, 2, 4, 8],
            max_ban_seconds: 604_800,
            cooldown_seconds: 30,
            fail_codes: vec![401, 403],
        },
        RateLimitProfileTemplate {
            id: 3,
            name: "Path Scanning".into(),
            description: "20 requests in 60 seconds → 5min ban with escalation. Un humano jamas genera 20 errores 404 en 1 minuto.".into(),
            max_requests: 20,
            window_seconds: 60,
            ban_seconds: 300,
            escalation_enabled: true,
            escalation_multipliers: vec![1, 2, 4, 8],
            max_ban_seconds: 86_400,
            cooldown_seconds: 30,
            fail_codes: vec![403, 404],
        },
        RateLimitProfileTemplate {
            id: 4,
            name: "API Abuse".into(),
            description: "100 requests in 1 minute → 5min ban with escalation".into(),
            max_requests: 100,
            window_seconds: 60,
            ban_seconds: 300,
            escalation_enabled: true,
            escalation_multipliers: vec![1, 2, 4, 8],
            max_ban_seconds: 86_400,
            cooldown_seconds: 30,
            fail_codes: vec![401, 403, 429],
        },
        RateLimitProfileTemplate {
            id: 5,
            name: "Scraping".into(),
            description: "60 requests in 1 minute → 5min ban with escalation".into(),
            max_requests: 60,
            window_seconds: 60,
            ban_seconds: 300,
            escalation_enabled: true,
            escalation_multipliers: vec![1, 2, 4, 8],
            max_ban_seconds: 86_400,
            cooldown_seconds: 7,
            fail_codes: vec![403, 429, 500],
        },
        RateLimitProfileTemplate {
            id: 6,
            name: "Health & Webhooks".into(),
            description: "100 requests in 60 seconds → 1min ban".into(),
            max_requests: 100,
            window_seconds: 60,
            ban_seconds: 60,
            escalation_enabled: false,
            escalation_multipliers: vec![1],
            max_ban_seconds: 60,
            cooldown_seconds: 30,
            fail_codes: vec![500, 502, 503],
        },
        RateLimitProfileTemplate {
            id: 7,
            name: "Recidive".into(),
            description: "3 reincidences in 48h → 7-day ban (max 30 days)".into(),
            max_requests: 3,
            window_seconds: 172_800,
            ban_seconds: 604_800,
            escalation_enabled: true,
            escalation_multipliers: vec![1, 2, 4, 8],
            max_ban_seconds: 2_592_000,
            cooldown_seconds: 60,
            fail_codes: vec![403, 429],
        },
        RateLimitProfileTemplate {
            id: 9,
            name: "Scanner Aggressive".into(),
            description: "50 requests in 10 seconds → 30min ban with escalation. Para escaneres rapidos incluso con retardo aleatorio.".into(),
            max_requests: 50,
            window_seconds: 10,
            ban_seconds: 1800,
            escalation_enabled: true,
            escalation_multipliers: vec![1, 2, 4, 8],
            max_ban_seconds: 604_800,
            cooldown_seconds: 30,
            fail_codes: vec![403, 404, 405, 500],
        },
        RateLimitProfileTemplate {
            id: 8,
            name: "Global Shield".into(),
            description: "Catch-all: 300 errores en 60s → 5min ban with escalation. Sin filtros, matchea todo. Atrapa cualquier IP que genere demasiados errores.".into(),
            max_requests: 300,
            window_seconds: 60,
            ban_seconds: 300,
            escalation_enabled: true,
            escalation_multipliers: vec![1, 2, 4, 8],
            max_ban_seconds: 86_400,
            cooldown_seconds: 30,
            fail_codes: vec![403, 404, 429, 500, 502, 503],
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_waf_constructor_sets_correct_defaults() {
        let t = RuleTemplate::waf("Test", "A test template");
        assert_eq!(t.name, "Test");
        assert_eq!(t.description, "A test template");
        assert_eq!(t.pipeline, "waf");
        assert!(!t.allow);
        assert!(t.ip_address.is_none());
        assert!(t.path.is_none());
        assert!(t.query.is_none());
        assert!(t.country_code.is_none());
        assert!(t.user_agent.is_none());
        assert!(t.method.is_none());
        assert!(!t.must_have);
        assert_eq!(t.weight, 100);
        assert!(t.rate_limit_profile_id.is_none());
        assert!(t.is_tor.is_none());
    }

    #[test]
    fn test_jail_constructor_sets_correct_defaults() {
        let t = RuleTemplate::jail("Test", "A jail template");
        assert_eq!(t.name, "Test");
        assert_eq!(t.description, "A jail template");
        assert_eq!(t.pipeline, "jail");
        assert!(t.allow);
        assert!(t.rate_limit_profile_id.is_none());
    }

    #[test]
    fn test_builder_methods() {
        let t = RuleTemplate::waf("Test", "Desc")
            .category("seguridad")
            .severity("critico")
            .path(r"^/admin")
            .query(r"action=delete")
            .country_code("RU")
            .user_agent(r"curl")
            .method("POST")
            .ip_address("10.0.0.1")
            .fqdn("example.com")
            .city_name("Moscow")
            .country_name("Russia")
            .referer("evil.com")
            .content_type("application/x-www-form-urlencoded")
            .accept_language("ru")
            .x_request_id("abc")
            .allow(true)
            .requires_fqdn(true)
            .must_have(true)
            .weight(50);

        assert_eq!(t.category, "seguridad");
        assert_eq!(t.severity, "critico");
        assert_eq!(t.path.as_deref(), Some(r"^/admin"));
        assert_eq!(t.query.as_deref(), Some(r"action=delete"));
        assert_eq!(t.country_code.as_deref(), Some("RU"));
        assert_eq!(t.user_agent.as_deref(), Some(r"curl"));
        assert_eq!(t.method.as_deref(), Some("POST"));
        assert_eq!(t.ip_address.as_deref(), Some("10.0.0.1"));
        assert_eq!(t.fqdn.as_deref(), Some("example.com"));
        assert_eq!(t.city_name.as_deref(), Some("Moscow"));
        assert_eq!(t.country_name.as_deref(), Some("Russia"));
        assert_eq!(t.referer.as_deref(), Some("evil.com"));
        assert_eq!(
            t.content_type.as_deref(),
            Some("application/x-www-form-urlencoded")
        );
        assert_eq!(t.accept_language.as_deref(), Some("ru"));
        assert_eq!(t.x_request_id.as_deref(), Some("abc"));
        assert!(t.allow);
        assert!(t.requires_fqdn);
        assert!(t.must_have);
        assert_eq!(t.weight, 50);
        assert!(t.is_tor.is_none());
    }

    #[test]
    fn test_rate_limit_profile_builder() {
        let t = RuleTemplate::jail("Test", "Desc").rate_limit_profile(1, "Auth Brute Force");
        assert_eq!(t.rate_limit_profile_id, Some(1));
        assert_eq!(
            t.rate_limit_profile_name.as_deref(),
            Some("Auth Brute Force")
        );
    }

    #[test]
    fn test_waf_template_count() {
        let all = all_rule_templates();
        let waf: Vec<_> = all.iter().filter(|t| t.pipeline == "waf").collect();
        assert_eq!(
            waf.len(),
            24,
            "Expected 24 WAF templates, got {}",
            waf.len()
        );
    }

    #[test]
    fn test_jail_template_count() {
        let all = all_rule_templates();
        let jail: Vec<_> = all.iter().filter(|t| t.pipeline == "jail").collect();
        assert_eq!(
            jail.len(),
            18,
            "Expected 18 JAIL templates, got {}",
            jail.len()
        );
    }

    #[test]
    fn test_no_jail_template_has_allow_false() {
        let all = all_rule_templates();
        for t in &all {
            if t.pipeline == "jail" {
                assert!(t.allow, "JAIL template '{}' has allow=false", t.name);
            }
        }
    }

    #[test]
    fn test_must_have_templates_appear_first_in_waf() {
        let all = all_rule_templates();
        let waf: Vec<_> = all.iter().filter(|t| t.pipeline == "waf").collect();
        let mut found_non_must_have = false;
        for t in &waf {
            if t.must_have {
                assert!(
                    !found_non_must_have,
                    "Must-have '{}' appears after non-must-have",
                    t.name
                );
            } else {
                found_non_must_have = true;
            }
        }
    }

    #[test]
    fn test_severity_values_are_valid() {
        let valid = ["critico", "alto", "medio", "bajo"];
        let all = all_rule_templates();
        for t in &all {
            assert!(
                valid.contains(&t.severity.as_str()),
                "Template '{}' has invalid severity '{}'",
                t.name,
                t.severity
            );
        }
    }

    #[test]
    fn test_rate_limit_profile_count() {
        let profiles = all_rate_limit_profile_templates();
        assert_eq!(profiles.len(), 9, "Expected 9 rate limit profiles");
    }

    #[test]
    fn test_health_webhooks_template_exists() {
        let all = all_rule_templates();
        let found = all.iter().any(|t| t.name == "Health & Webhooks");
        assert!(found, "Health & Webhooks template not found");
    }

    #[test]
    fn test_recidive_template_exists() {
        let all = all_rule_templates();
        let found = all.iter().any(|t| t.name == "Recidive");
        assert!(found, "Recidive template not found");
    }

    #[test]
    fn test_tor_exit_nodes_template_exists() {
        let all = all_rule_templates();
        let found = all.iter().any(|t| t.name == "Tor exit nodes");
        assert!(found, "Tor exit nodes template not found");
    }

    #[test]
    fn test_tor_exit_nodes_template_properties() {
        let all = all_rule_templates();
        let t = all.iter().find(|t| t.name == "Tor exit nodes");
        assert!(t.is_some(), "Tor exit nodes template not found");
        let t = t.unwrap();
        assert_eq!(
            t.is_tor,
            Some(true),
            "Tor exit nodes should have is_tor=true"
        );
        assert!(!t.allow, "Tor exit nodes should have allow=false");
        assert_eq!(t.pipeline, "waf", "Tor exit nodes should be WAF");
        assert!(!t.must_have, "Tor exit nodes should not be must_have");
        assert_eq!(t.weight, 300, "Tor exit nodes should have weight 300");
        assert_eq!(t.category, "tor", "Tor exit nodes category should be 'tor'");
    }

    #[test]
    fn test_tor_exit_nodes_appears_after_geo_blocking() {
        let all = all_rule_templates();
        let waf: Vec<_> = all.iter().filter(|t| t.pipeline == "waf").collect();
        let geo_pos = waf.iter().position(|t| t.name == "Geo blocking");
        let tor_pos = waf.iter().position(|t| t.name == "Tor exit nodes");
        assert!(geo_pos.is_some(), "Geo blocking template not found");
        assert!(tor_pos.is_some(), "Tor exit nodes template not found");
        assert!(
            tor_pos.unwrap() > geo_pos.unwrap(),
            "Tor exit nodes should appear after Geo blocking"
        );
    }

    #[test]
    fn test_phpunit_probe_is_waf() {
        let all = all_rule_templates();
        let t = all.iter().find(|t| t.name.contains("PHPUnit"));
        assert!(t.is_some(), "PHPUnit probe template not found");
        assert_eq!(t.unwrap().pipeline, "waf", "PHPUnit probe should be WAF");
        assert!(!t.unwrap().allow, "PHPUnit probe should have allow=false");
    }

    #[test]
    fn test_xmlrpc_templates_are_waf() {
        let all = all_rule_templates();
        for t in all
            .iter()
            .filter(|t| t.name.to_lowercase().contains("xmlrpc"))
        {
            assert_eq!(
                t.pipeline, "waf",
                "xmlrpc template '{}' should be WAF",
                t.name
            );
            assert!(
                !t.allow,
                "xmlrpc template '{}' should have allow=false",
                t.name
            );
        }
    }
}
