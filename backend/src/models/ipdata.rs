use serde::{Serialize, Deserialize};
use maxminddb::{
    Reader,
    geoip2,
};
use tracing::debug;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct IPData {
    pub ip_address: String,
    pub city_name: Option<String>,
    pub country_name: Option<String>,
    pub country_code: Option<String>,
}

impl IPData {
    pub fn complete(maxmind_db: Reader<Vec<u8>>, ip_address: &str) -> Self{
        let ip = ip_address.parse().unwrap();
        match maxmind_db.lookup::<geoip2::City>(ip).unwrap_or_default() {
            Some(city) => {
                debug!("city: {:?}", city);
                Self {
                    ip_address: ip_address.to_string(),
                    city_name: None,
                    country_name: None,
                    country_code: None,
                }
            },
            None => {
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
