use thiserror::Error;

#[derive(Error, Debug)]
pub enum Ddns6Error {
    #[error("Configuration error: {0}")]
    Config(String),

    #[error("IPv6 parsing error: {0}")]
    Ipv6Parse(String),

    #[allow(dead_code)]
    #[error("Hostname not found in configuration: {0}")]
    HostnameNotFound(String),

    #[error("Invalid Interface ID: {0}")]
    InvalidInterfaceId(String),

    #[error("Cloudflare API error: {0}")]
    CloudflareApi(String),

    #[error("HTTP request error: {0}")]
    HttpRequest(#[from] reqwest::Error),

    #[allow(dead_code)]
    #[error("Invalid DynDNS2 request: {0}")]
    InvalidDynDns2Request(String),

    #[allow(dead_code)]
    #[error("State management error: {0}")]
    State(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, Ddns6Error>;
