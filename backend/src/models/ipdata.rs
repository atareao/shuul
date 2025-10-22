use maxminddb::{Reader, geoip2};
use serde::{Deserialize, Serialize};
use tracing::{debug, error};

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
            Ok(ip) => {
                debug!("Look data for ip: {:?}", ip);
                match maxmind_db.lookup::<geoip2::City>(ip).unwrap_or_default() {
                    Some(result) => {
                        debug!("result: {:?}", result);
                        let country = result.country.as_ref();
                        Self {
                            ip_address: ip_address.to_string(),
                            city_name: result
                                .city
                                .and_then(|c| c.names)
                                .and_then(|n| n.get("en").cloned())
                                .map(|s| s.to_string()),
                            country_name: country
                                .and_then(|c| c.names.clone())
                                .and_then(|n| n.get("en").cloned())
                                .map(|s| s.to_string()),
                            country_code: result
                                .country
                                .and_then(|c| c.iso_code)
                                .map(|s| s.to_string()),
                        }
                    }
                    None => Self {
                        ip_address: ip_address.to_string(),
                        city_name: None,
                        country_name: None,
                        country_code: None,
                    },
                }
            }
            Err(e) => {
                error!("Look data for ip: {:?}: {}", ip_address, e);
                Self {
                    ip_address: ip_address.to_string(),
                    city_name: None,
                    country_name: None,
                    country_code: None,
                }
            }
        }
    }
}
