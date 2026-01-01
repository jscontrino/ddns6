use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::net::Ipv6Addr;
use std::path::Path;

use crate::error::{Ddns6Error, Result};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Config {
    pub server: ServerConfig,
    pub cloudflare: CloudflareConfig,
    #[serde(rename = "hosts")]
    pub hosts: Vec<HostMapping>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ServerConfig {
    pub bind_address: String,
    #[serde(default = "default_workers")]
    pub workers: usize,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CloudflareConfig {
    pub api_token: String,
    pub zone_id: String,
    #[serde(default = "default_ttl")]
    pub ttl: u32,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct HostMapping {
    pub hostname: String,
    pub interface_id: String,
}

fn default_workers() -> usize {
    4
}

fn default_ttl() -> u32 {
    300
}

impl Config {
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let content = fs::read_to_string(path)
            .map_err(|e| Ddns6Error::Config(format!("Failed to read config file: {}", e)))?;

        let config: Config = toml::from_str(&content)
            .map_err(|e| Ddns6Error::Config(format!("Failed to parse config file: {}", e)))?;

        config.validate()?;
        Ok(config)
    }

    fn validate(&self) -> Result<()> {
        if self.server.bind_address.is_empty() {
            return Err(Ddns6Error::Config(
                "bind_address cannot be empty".to_string(),
            ));
        }

        if self.cloudflare.api_token.is_empty() {
            return Err(Ddns6Error::Config(
                "cloudflare.api_token cannot be empty".to_string(),
            ));
        }

        if self.cloudflare.zone_id.is_empty() {
            return Err(Ddns6Error::Config(
                "cloudflare.zone_id cannot be empty".to_string(),
            ));
        }

        if self.hosts.is_empty() {
            return Err(Ddns6Error::Config(
                "At least one host mapping must be configured".to_string(),
            ));
        }

        for host in &self.hosts {
            if host.hostname.is_empty() {
                return Err(Ddns6Error::Config("hostname cannot be empty".to_string()));
            }

            self.validate_interface_id(&host.interface_id)?;
        }

        let mut seen_hostnames = HashMap::new();
        for host in &self.hosts {
            if seen_hostnames.contains_key(&host.hostname) {
                return Err(Ddns6Error::Config(format!(
                    "Duplicate hostname: {}",
                    host.hostname
                )));
            }
            seen_hostnames.insert(host.hostname.clone(), ());
        }

        Ok(())
    }

    fn validate_interface_id(&self, iid: &str) -> Result<()> {
        if iid.parse::<Ipv6Addr>().is_ok() {
            return Ok(());
        }

        let test_addr = format!("2001:db8::{}", iid);
        if test_addr.parse::<Ipv6Addr>().is_ok() {
            return Ok(());
        }

        Err(Ddns6Error::InvalidInterfaceId(format!(
            "Invalid interface ID format: {}",
            iid
        )))
    }

    #[allow(dead_code)]
    pub fn get_host(&self, hostname: &str) -> Option<&HostMapping> {
        self.hosts.iter().find(|h| h.hostname == hostname)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_interface_id() {
        let config = Config {
            server: ServerConfig {
                bind_address: "0.0.0.0:8080".to_string(),
                workers: 4,
            },
            cloudflare: CloudflareConfig {
                api_token: "test".to_string(),
                zone_id: "test".to_string(),
                ttl: 300,
            },
            hosts: vec![],
        };

        assert!(config.validate_interface_id("::1").is_ok());
        assert!(config.validate_interface_id("::2").is_ok());
        assert!(config
            .validate_interface_id("::a1b2:c3d4:e5f6:7890")
            .is_ok());
        assert!(config.validate_interface_id("1").is_ok());
        assert!(config.validate_interface_id("1234:5678:90ab:cdef").is_ok());
    }

    #[test]
    fn test_get_host() {
        let config = Config {
            server: ServerConfig {
                bind_address: "0.0.0.0:8080".to_string(),
                workers: 4,
            },
            cloudflare: CloudflareConfig {
                api_token: "test".to_string(),
                zone_id: "test".to_string(),
                ttl: 300,
            },
            hosts: vec![
                HostMapping {
                    hostname: "device1.example.com".to_string(),
                    interface_id: "::1".to_string(),
                },
                HostMapping {
                    hostname: "device2.example.com".to_string(),
                    interface_id: "::2".to_string(),
                },
            ],
        };

        assert!(config.get_host("device1.example.com").is_some());
        assert!(config.get_host("device2.example.com").is_some());
        assert!(config.get_host("nonexistent.example.com").is_none());
    }

    #[test]
    fn test_validate_empty_bind_address() {
        let config = Config {
            server: ServerConfig {
                bind_address: "".to_string(),
                workers: 4,
            },
            cloudflare: CloudflareConfig {
                api_token: "test".to_string(),
                zone_id: "test".to_string(),
                ttl: 300,
            },
            hosts: vec![HostMapping {
                hostname: "test.example.com".to_string(),
                interface_id: "::1".to_string(),
            }],
        };

        assert!(config.validate().is_err());
    }

    #[test]
    fn test_validate_empty_api_token() {
        let config = Config {
            server: ServerConfig {
                bind_address: "0.0.0.0:8080".to_string(),
                workers: 4,
            },
            cloudflare: CloudflareConfig {
                api_token: "".to_string(),
                zone_id: "test".to_string(),
                ttl: 300,
            },
            hosts: vec![HostMapping {
                hostname: "test.example.com".to_string(),
                interface_id: "::1".to_string(),
            }],
        };

        assert!(config.validate().is_err());
    }

    #[test]
    fn test_validate_empty_zone_id() {
        let config = Config {
            server: ServerConfig {
                bind_address: "0.0.0.0:8080".to_string(),
                workers: 4,
            },
            cloudflare: CloudflareConfig {
                api_token: "test".to_string(),
                zone_id: "".to_string(),
                ttl: 300,
            },
            hosts: vec![HostMapping {
                hostname: "test.example.com".to_string(),
                interface_id: "::1".to_string(),
            }],
        };

        assert!(config.validate().is_err());
    }

    #[test]
    fn test_validate_no_hosts() {
        let config = Config {
            server: ServerConfig {
                bind_address: "0.0.0.0:8080".to_string(),
                workers: 4,
            },
            cloudflare: CloudflareConfig {
                api_token: "test".to_string(),
                zone_id: "test".to_string(),
                ttl: 300,
            },
            hosts: vec![],
        };

        assert!(config.validate().is_err());
    }

    #[test]
    fn test_validate_empty_hostname() {
        let config = Config {
            server: ServerConfig {
                bind_address: "0.0.0.0:8080".to_string(),
                workers: 4,
            },
            cloudflare: CloudflareConfig {
                api_token: "test".to_string(),
                zone_id: "test".to_string(),
                ttl: 300,
            },
            hosts: vec![HostMapping {
                hostname: "".to_string(),
                interface_id: "::1".to_string(),
            }],
        };

        assert!(config.validate().is_err());
    }

    #[test]
    fn test_validate_invalid_interface_id() {
        let config = Config {
            server: ServerConfig {
                bind_address: "0.0.0.0:8080".to_string(),
                workers: 4,
            },
            cloudflare: CloudflareConfig {
                api_token: "test".to_string(),
                zone_id: "test".to_string(),
                ttl: 300,
            },
            hosts: vec![HostMapping {
                hostname: "test.example.com".to_string(),
                interface_id: "invalid::xyz::123".to_string(),
            }],
        };

        assert!(config.validate().is_err());
    }

    #[test]
    fn test_validate_duplicate_hostname() {
        let config = Config {
            server: ServerConfig {
                bind_address: "0.0.0.0:8080".to_string(),
                workers: 4,
            },
            cloudflare: CloudflareConfig {
                api_token: "test".to_string(),
                zone_id: "test".to_string(),
                ttl: 300,
            },
            hosts: vec![
                HostMapping {
                    hostname: "test.example.com".to_string(),
                    interface_id: "::1".to_string(),
                },
                HostMapping {
                    hostname: "test.example.com".to_string(),
                    interface_id: "::2".to_string(),
                },
            ],
        };

        assert!(config.validate().is_err());
    }

    #[test]
    fn test_defaults() {
        assert_eq!(default_workers(), 4);
        assert_eq!(default_ttl(), 300);
    }

    #[test]
    fn test_valid_config() {
        let config = Config {
            server: ServerConfig {
                bind_address: "127.0.0.1:8080".to_string(),
                workers: 2,
            },
            cloudflare: CloudflareConfig {
                api_token: "my-api-token".to_string(),
                zone_id: "my-zone-id".to_string(),
                ttl: 600,
            },
            hosts: vec![
                HostMapping {
                    hostname: "device1.example.com".to_string(),
                    interface_id: "::1".to_string(),
                },
                HostMapping {
                    hostname: "device2.example.com".to_string(),
                    interface_id: "::ffff:192.168.1.1".to_string(),
                },
            ],
        };

        assert!(config.validate().is_ok());
    }
}
