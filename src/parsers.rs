use std::{
    collections::{HashMap, HashSet},
    fs::File,
    io::Read,
    path::Path,
    str::FromStr,
    time::{Duration, Instant},
};

use eui48::MacAddress;
use serde::Deserialize;

use crate::{error::Error, expiry::ExpireSet};

fn is_valid_mac(mac: MacAddress) -> bool {
    mac.is_universal() && mac.is_unicast()
}

macro_rules! unwrap_option_or_continue {
    ($val:expr) => {
        if let Some(v) = $val {
            v
        } else {
            continue;
        }
    };
}

macro_rules! unwrap_result_or_continue {
    ($val:expr) => {
        if let Ok(v) = $val {
            v
        } else {
            continue;
        }
    };
}

fn parse_port_data(data: String, _format: PortDataFormat) -> Result<ExpireSet<MacAddress>, Error> {
    let mut set = ExpireSet::default();
    let expiry = Instant::now() + Duration::from_secs(5);

    for line in data.split('\n') {
        if line.len() != 17 {
            continue;
        }

        if line.chars().nth(2) != Some(':') {
            continue;
        }

        let mac = unwrap_result_or_continue!(MacAddress::from_str(line));
        log::trace!("hostapd reported hardware {}", mac);
        set.insert(mac, expiry);
    }

    Ok(set)
}

#[derive(Deserialize, Clone, Copy)]
pub enum PortDataFormat {
    #[serde(rename = "hostapd")]
    HostApd,
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum PortPoller {
    File {
        file: String,
        format: PortDataFormat,
    },
}

impl PortPoller {
    pub fn poll(&self, root: &Path) -> Result<ExpireSet<MacAddress>, Error> {
        let (data, format) = match self {
            PortPoller::File { file, format } => {
                let path = root.join(file);

                let mut file = File::open(path).map_err(Error::IoError)?;
                let mut data = String::new();
                file.read_to_string(&mut data).map_err(Error::IoError)?;
                (data, *format)
            }
        };

        parse_port_data(data, format)
    }
}

fn parse_device_data(
    data: String,
    format: DeviceDataFormat,
) -> Result<HashMap<String, ExpireSet<MacAddress>>, Error> {
    let expiry = Instant::now() + Duration::from_secs(5);
    let mut map: HashMap<String, ExpireSet<MacAddress>> = HashMap::new();

    match format {
        DeviceDataFormat::ForwardDb => {
            for line in data.split('\n') {
                let mut parts = line.split(' ');

                let addr = unwrap_option_or_continue!(parts.next());
                let mac = unwrap_result_or_continue!(MacAddress::from_str(addr));
                if !is_valid_mac(mac) {
                    continue;
                }

                if parts.next() != Some("dev") {
                    log::warn!("fdb line appears invalid, missing dev.");
                    continue;
                }

                let port = unwrap_option_or_continue!(parts.next());
                let flags: HashSet<&str> = parts.collect();
                if flags.contains("permanent") || flags.contains("self") {
                    continue;
                }

                log::trace!("fdb reported hardware {}", mac);

                if let Some(set) = map.get_mut(port) {
                    set.insert(mac, expiry);
                } else {
                    let mut set = ExpireSet::default();
                    set.insert(mac, expiry);
                    map.insert(port.to_owned(), set);
                }
            }
        }
        DeviceDataFormat::SwConfig => {
            for line in data.split('\n') {
                let mut parts = line.split(' ');

                if parts.next() != Some("Port") {
                    log::warn!("swconfig line appears invalid, missing port.");
                    continue;
                }

                let port = unwrap_option_or_continue!(parts.next()).trim_end_matches(':');

                if parts.next() != Some("MAC") {
                    log::warn!("swconfig line appears invalid, missing mac.");
                    continue;
                }

                let addr = unwrap_option_or_continue!(parts.next());
                let mac = unwrap_result_or_continue!(MacAddress::from_str(addr));
                if !is_valid_mac(mac) {
                    continue;
                }

                log::trace!("swconfig reported hardware {}", mac);

                if let Some(set) = map.get_mut(port) {
                    set.insert(mac, expiry);
                } else {
                    let mut set = ExpireSet::default();
                    set.insert(mac, expiry);
                    map.insert(port.to_owned(), set);
                }
            }
        }
    }
    Ok(map)
}

#[derive(Deserialize, Clone, Copy)]
pub enum DeviceDataFormat {
    #[serde(rename = "fdb")]
    ForwardDb,
    #[serde(rename = "swc")]
    SwConfig,
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum DevicePoller {
    File {
        file: String,
        format: DeviceDataFormat,
    },
}

impl DevicePoller {
    pub fn poll(&self, root: &Path) -> Result<HashMap<String, ExpireSet<MacAddress>>, Error> {
        let (data, format) = match self {
            DevicePoller::File { file, format } => {
                let path = root.join(file);

                let mut file = File::open(path).map_err(Error::IoError)?;
                let mut data = String::new();
                file.read_to_string(&mut data).map_err(Error::IoError)?;
                (data, *format)
            }
        };

        parse_device_data(data, format)
    }
}
