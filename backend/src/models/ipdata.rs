//! # Datos de geolocalización IP
//!
//! [`IPData`] almacena la información de geolocalización obtenida
//! de la base de datos `MaxMind` `GeoIP2` para una dirección IP dada.
//!
//! [`GeoIpService`] envuelve el `Reader` de MaxMind con un cache LRU+TTL
//! para evitar consultas repetidas a la base de datos GeoIP.

use maxminddb::{Reader, geoip2};
use moka::sync::Cache;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tracing::{error, trace};

/// Tiempo de vida de las entradas del cache GeoIP.
const GEOIP_CACHE_TTL: Duration = Duration::from_secs(3600); // 1 hora
/// Número máximo de entradas en el cache GeoIP.
const GEOIP_CACHE_MAX_CAPACITY: u64 = 10_000;

/// Información de geolocalización asociada a una IP.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct IPData {
    pub ip_address: String,
    pub city_name: Option<String>,
    pub country_name: Option<String>,
    pub country_code: Option<String>,
}

/// Servicio de geolocalización con cache LRU+TTL.
///
/// Encapsula el `Reader` de MaxMind y un cache concurrente (`moka`)
/// que evita consultas repetidas a la base de datos GeoIP para IPs
/// ya resueltas. Los hits de cache son operaciones de tabla hash
/// (nanosegundos) frente a la búsqueda binaria en el archivo mmdb
/// (microsegundos).
pub struct GeoIpService {
    reader: Reader<Vec<u8>>,
    cache: Cache<String, IPData>,
}

impl GeoIpService {
    /// Crea un nuevo `GeoIpService` con un cache LRU+TTL.
    pub fn new(reader: Reader<Vec<u8>>) -> Self {
        let cache = Cache::builder()
            .time_to_live(GEOIP_CACHE_TTL)
            .max_capacity(GEOIP_CACHE_MAX_CAPACITY)
            .build();
        Self { reader, cache }
    }

    /// Resuelve la geolocalización de una IP, usando el cache si es posible.
    pub fn lookup(&self, ip: &str) -> IPData {
        if let Some(data) = self.cache.get(ip) {
            return data;
        }

        let data = IPData::complete(&self.reader, ip);
        // Solo cachear IPs con datos (evita cachear IPs inválidas/privadas).
        if data.city_name.is_some() || data.country_name.is_some() || data.country_code.is_some() {
            self.cache.insert(ip.to_string(), data.clone());
        }
        data
    }

    /// Acceso directo al `Reader` de MaxMind (para consultas puntuales admin).
    #[allow(dead_code)]
    pub fn reader(&self) -> &Reader<Vec<u8>> {
        &self.reader
    }

    /// Número de entradas actualmente en el cache (para diagnóstico).
    #[allow(dead_code)]
    pub fn cache_len(&self) -> u64 {
        self.cache.weighted_size()
    }
}

impl IPData {
    pub fn complete(maxmind_db: &Reader<Vec<u8>>, ip_address: &str) -> Self {
        match ip_address.parse() {
            Ok(ip) => match maxmind_db.lookup(ip) {
                Ok(result) if result.has_data() => match result.decode::<geoip2::City>() {
                    Ok(Some(city)) => {
                        trace!("result: {:?}", city);
                        Self {
                            ip_address: ip_address.to_string(),
                            city_name: if city.city.is_empty() {
                                None
                            } else {
                                city.city
                                    .names
                                    .english
                                    .map(std::string::ToString::to_string)
                            },
                            country_name: if city.country.is_empty() {
                                None
                            } else {
                                city.country
                                    .names
                                    .english
                                    .map(std::string::ToString::to_string)
                            },
                            country_code: if city.country.is_empty() {
                                None
                            } else {
                                city.country.iso_code.map(std::string::ToString::to_string)
                            },
                        }
                    },
                    _ => Self {
                        ip_address: ip_address.to_string(),
                        city_name: None,
                        country_name: None,
                        country_code: None,
                    },
                },
                _ => Self {
                    ip_address: ip_address.to_string(),
                    city_name: None,
                    country_name: None,
                    country_code: None,
                },
            },
            Err(e) => {
                error!("Look data for ip: {:?}: {}", ip_address, e);
                Self {
                    ip_address: ip_address.to_string(),
                    city_name: None,
                    country_name: None,
                    country_code: None,
                }
            },
        }
    }
}
