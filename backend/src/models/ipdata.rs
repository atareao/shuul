//! # Datos de geolocalización IP
//!
//! [`IPData`] almacena la información de geolocalización obtenida
//! de la base de datos `MaxMind` `GeoIP2` para una dirección IP dada.

use maxminddb::{Reader, geoip2};
use serde::{Deserialize, Serialize};
use tracing::{debug, error};

/// Información de geolocalización asociada a una IP.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct IPData {
    pub ip_address: String,
    pub city_name: Option<String>,
    pub country_name: Option<String>,
    pub country_code: Option<String>,
}

impl IPData {
    pub fn complete(maxmind_db: &Reader<Vec<u8>>, ip_address: &str) -> Self {
        debug!("Look data for ip address: {}", ip_address);
        match ip_address.parse() {
            Ok(ip) => match maxmind_db.lookup(ip) {
                Ok(result) if result.has_data() => match result.decode::<geoip2::City>() {
                    Ok(Some(city)) => {
                        debug!("result: {:?}", city);
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
