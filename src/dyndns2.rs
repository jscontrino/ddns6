use axum::{
    extract::{Query, State as AxumState},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Deserialize;
use std::net::Ipv6Addr;
use std::sync::Arc;
use tracing::{debug, error, info, warn};

use crate::cloudflare::CloudflareClient;
use crate::config::Config;
use crate::error::Ddns6Error;
use crate::ipv6::Ipv6Prefix;
use crate::state::StateCache;

#[derive(Debug, Deserialize)]
pub struct UpdateQuery {
    prefix: String,
}

#[derive(Debug, Clone)]
pub struct AppState {
    pub config: Arc<Config>,
    pub state_cache: StateCache,
    pub cloudflare_client: Arc<CloudflareClient>,
}

pub enum DynDns2Response {
    Good(Vec<String>),
    NoChg(Vec<String>),
    PartialSuccess(Vec<String>, Vec<String>),
    #[allow(dead_code)]
    BadAgent,
    #[allow(dead_code)]
    Abuse,
    Error(String),
}

impl IntoResponse for DynDns2Response {
    fn into_response(self) -> Response {
        let (status, body) = match self {
            DynDns2Response::Good(hosts) => (StatusCode::OK, format!("good {}", hosts.join(", "))),
            DynDns2Response::NoChg(hosts) => {
                (StatusCode::OK, format!("nochg {}", hosts.join(", ")))
            }
            DynDns2Response::PartialSuccess(success, failed) => (
                StatusCode::OK,
                format!(
                    "partial success: {} | failed: {}",
                    success.join(", "),
                    failed.join(", ")
                ),
            ),
            DynDns2Response::BadAgent => (StatusCode::OK, "badagent".to_string()),
            DynDns2Response::Abuse => (StatusCode::OK, "abuse".to_string()),
            DynDns2Response::Error(msg) => (StatusCode::OK, format!("911 {}", msg)),
        };

        (status, body).into_response()
    }
}

pub async fn handle_update(
    AxumState(state): AxumState<AppState>,
    Query(params): Query<UpdateQuery>,
) -> DynDns2Response {
    info!("Received update request for all hosts");
    debug!("Update parameters: {:?}", params);

    let client_ipv6 = match extract_ipv6_address(&params) {
        Ok(addr) => addr,
        Err(e) => {
            error!("Failed to extract IPv6 address: {}", e);
            return DynDns2Response::Error("Invalid IPv6 address".to_string());
        }
    };

    debug!("Client IPv6 address: {}", client_ipv6);

    let prefix = match Ipv6Prefix::extract_from_address(client_ipv6, 64) {
        Ok(p) => p,
        Err(e) => {
            error!("Failed to extract prefix: {}", e);
            return DynDns2Response::Error("Failed to extract prefix".to_string());
        }
    };

    info!(
        "Extracted prefix: {}/{}, updating all {} host(s)",
        prefix.network(),
        prefix.prefix_len(),
        state.config.hosts.len()
    );

    let mut updated_hosts = Vec::new();
    let mut unchanged_hosts = Vec::new();
    let mut failed_hosts = Vec::new();

    for host in &state.config.hosts {
        let final_address = match prefix.combine_with_interface_id(&host.interface_id) {
            Ok(addr) => addr,
            Err(e) => {
                error!(
                    "Failed to combine prefix with interface ID for {}: {}",
                    host.hostname, e
                );
                failed_hosts.push(host.hostname.clone());
                continue;
            }
        };

        debug!(
            "Computed address for {}: {} (prefix {} + interface_id {})",
            host.hostname,
            final_address,
            prefix.network(),
            host.interface_id
        );

        let has_changed = state
            .state_cache
            .has_changed(&host.hostname, final_address)
            .await;

        if !has_changed {
            info!("Address for {} has not changed, skipping", host.hostname);
            unchanged_hosts.push(format!("{}={}", host.hostname, final_address));
            continue;
        }

        info!(
            "Address for {} has changed to {}, updating Cloudflare",
            host.hostname, final_address
        );

        match state
            .cloudflare_client
            .update_aaaa_record(&host.hostname, final_address)
            .await
        {
            Ok(_) => {
                state
                    .state_cache
                    .update(host.hostname.clone(), final_address)
                    .await;
                info!(
                    "Successfully updated {} to {}",
                    host.hostname, final_address
                );
                updated_hosts.push(format!("{}={}", host.hostname, final_address));
            }
            Err(e) => {
                error!("Failed to update Cloudflare for {}: {}", host.hostname, e);
                failed_hosts.push(host.hostname.clone());
            }
        }
    }

    if !failed_hosts.is_empty() && !updated_hosts.is_empty() {
        warn!(
            "Partial success: {} updated, {} failed",
            updated_hosts.len(),
            failed_hosts.len()
        );
        return DynDns2Response::PartialSuccess(updated_hosts, failed_hosts);
    }

    if !failed_hosts.is_empty() {
        error!("All updates failed");
        return DynDns2Response::Error(format!("Failed to update: {}", failed_hosts.join(", ")));
    }

    if !updated_hosts.is_empty() {
        info!("Successfully updated {} host(s)", updated_hosts.len());
        return DynDns2Response::Good(updated_hosts);
    }

    info!("No hosts needed updating (all unchanged)");
    DynDns2Response::NoChg(unchanged_hosts)
}

fn extract_ipv6_address(params: &UpdateQuery) -> Result<Ipv6Addr, Ddns6Error> {
    params
        .prefix
        .parse::<Ipv6Addr>()
        .map_err(|e| Ddns6Error::Ipv6Parse(format!("Failed to parse prefix parameter: {}", e)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_ipv6_from_prefix() {
        let params = UpdateQuery {
            prefix: "2001:db8::1".to_string(),
        };

        let result = extract_ipv6_address(&params).unwrap();
        assert_eq!(result, "2001:db8::1".parse::<Ipv6Addr>().unwrap());
    }

    #[test]
    fn test_extract_ipv6_invalid_prefix() {
        let params = UpdateQuery {
            prefix: "not-an-ip".to_string(),
        };

        assert!(extract_ipv6_address(&params).is_err());
    }

    #[test]
    fn test_extract_ipv6_various_formats() {
        let test_cases = vec![
            "2001:db8::1",
            "fe80::1",
            "::1",
            "::ffff:192.168.1.1",
            "2001:0db8:0000:0000:0000:0000:0000:0001",
        ];

        for addr_str in test_cases {
            let params = UpdateQuery {
                prefix: addr_str.to_string(),
            };

            assert!(
                extract_ipv6_address(&params).is_ok(),
                "Failed to parse: {}",
                addr_str
            );
        }
    }

    #[test]
    fn test_update_query_deserialization() {
        let query = UpdateQuery {
            prefix: "2001:db8::1".to_string(),
        };

        assert_eq!(query.prefix, "2001:db8::1");
    }

    #[test]
    fn test_dyndns2_response_good_format() {
        let hosts = vec!["device1.example.com=2001:db8::1".to_string()];
        let response = DynDns2Response::Good(hosts);

        let (parts, _body) = response.into_response().into_parts();
        let status = parts.status;

        assert_eq!(status, axum::http::StatusCode::OK);
    }

    #[test]
    fn test_dyndns2_response_nochg_format() {
        let hosts = vec!["device1.example.com=2001:db8::1".to_string()];
        let response = DynDns2Response::NoChg(hosts);

        let (parts, _body) = response.into_response().into_parts();
        let status = parts.status;

        assert_eq!(status, axum::http::StatusCode::OK);
    }

    #[test]
    fn test_dyndns2_response_error_codes() {
        let error = DynDns2Response::Error("Test error".to_string());
        let (parts, _body) = error.into_response().into_parts();
        let status = parts.status;
        assert_eq!(status, axum::http::StatusCode::OK);
    }

    #[test]
    fn test_dyndns2_response_partial_success() {
        let success = vec!["device1.example.com=2001:db8::1".to_string()];
        let failed = vec!["device2.example.com".to_string()];
        let response = DynDns2Response::PartialSuccess(success, failed);

        let (parts, _body) = response.into_response().into_parts();
        let status = parts.status;
        assert_eq!(status, axum::http::StatusCode::OK);
    }

    #[test]
    fn test_dyndns2_response_multiple_hosts() {
        let hosts = vec![
            "device1.example.com=2001:db8::1".to_string(),
            "device2.example.com=2001:db8::2".to_string(),
            "nas.example.com=2001:db8::100".to_string(),
        ];
        let response = DynDns2Response::Good(hosts.clone());

        let (parts, _body) = response.into_response().into_parts();
        let status = parts.status;

        assert_eq!(status, axum::http::StatusCode::OK);
    }
}
