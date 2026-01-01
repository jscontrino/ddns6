use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::net::Ipv6Addr;
use tracing::{debug, error, info};

use crate::error::{Ddns6Error, Result};

#[derive(Debug, Clone)]
pub struct CloudflareClient {
    client: Client,
    api_token: String,
    zone_id: String,
    ttl: u32,
}

#[derive(Debug, Serialize)]
struct CreateRecordRequest {
    #[serde(rename = "type")]
    record_type: String,
    name: String,
    content: String,
    ttl: u32,
    proxied: bool,
}

#[derive(Debug, Serialize)]
struct UpdateRecordRequest {
    #[serde(rename = "type")]
    record_type: String,
    name: String,
    content: String,
    ttl: u32,
    proxied: bool,
}

#[derive(Debug, Deserialize)]
struct CloudflareResponse<T> {
    success: bool,
    errors: Vec<CloudflareError>,
    #[allow(dead_code)]
    messages: Vec<String>,
    result: Option<T>,
}

#[derive(Debug, Deserialize)]
struct CloudflareError {
    code: u32,
    message: String,
}

#[derive(Debug, Deserialize)]
struct DnsRecord {
    id: String,
    #[allow(dead_code)]
    #[serde(rename = "type")]
    record_type: String,
    #[allow(dead_code)]
    name: String,
    #[allow(dead_code)]
    content: String,
    #[allow(dead_code)]
    ttl: u32,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct ListRecordsResult {
    result: Vec<DnsRecord>,
}

impl CloudflareClient {
    pub fn new(api_token: String, zone_id: String, ttl: u32) -> Self {
        Self {
            client: Client::new(),
            api_token,
            zone_id,
            ttl,
        }
    }

    pub async fn update_aaaa_record(&self, hostname: &str, ipv6_address: Ipv6Addr) -> Result<()> {
        info!("Updating AAAA record for {} to {}", hostname, ipv6_address);

        let existing_record = self.find_aaaa_record(hostname).await?;

        match existing_record {
            Some(record) => {
                debug!("Found existing record with ID: {}", record.id);
                self.update_record(&record.id, hostname, ipv6_address)
                    .await?;
            }
            None => {
                debug!("No existing record found, creating new one");
                self.create_record(hostname, ipv6_address).await?;
            }
        }

        info!("Successfully updated AAAA record for {}", hostname);
        Ok(())
    }

    async fn find_aaaa_record(&self, hostname: &str) -> Result<Option<DnsRecord>> {
        let url = format!(
            "https://api.cloudflare.com/client/v4/zones/{}/dns_records?type=AAAA&name={}",
            self.zone_id, hostname
        );

        debug!("Searching for existing AAAA record: {}", url);

        let response = self
            .client
            .get(&url)
            .bearer_auth(&self.api_token)
            .send()
            .await?;

        let status = response.status();
        let body = response.text().await?;

        if !status.is_success() {
            error!("Cloudflare API error (status {}): {}", status, body);
            return Err(Ddns6Error::CloudflareApi(format!(
                "Failed to list records: {} - {}",
                status, body
            )));
        }

        let list_response: CloudflareResponse<Vec<DnsRecord>> = serde_json::from_str(&body)
            .map_err(|e| {
                error!(
                    "Failed to parse Cloudflare response: {} - Body: {}",
                    e, body
                );
                Ddns6Error::CloudflareApi(format!("Failed to parse response: {}", e))
            })?;

        if !list_response.success {
            let error_msg = list_response
                .errors
                .iter()
                .map(|e| format!("{}: {}", e.code, e.message))
                .collect::<Vec<_>>()
                .join(", ");
            return Err(Ddns6Error::CloudflareApi(format!(
                "Cloudflare API returned errors: {}",
                error_msg
            )));
        }

        Ok(list_response
            .result
            .and_then(|records: Vec<DnsRecord>| records.into_iter().next()))
    }

    async fn create_record(&self, hostname: &str, ipv6_address: Ipv6Addr) -> Result<()> {
        let url = format!(
            "https://api.cloudflare.com/client/v4/zones/{}/dns_records",
            self.zone_id
        );

        let request = CreateRecordRequest {
            record_type: "AAAA".to_string(),
            name: hostname.to_string(),
            content: ipv6_address.to_string(),
            ttl: self.ttl,
            proxied: false,
        };

        debug!("Creating new AAAA record: {:?}", request);

        let response = self
            .client
            .post(&url)
            .bearer_auth(&self.api_token)
            .json(&request)
            .send()
            .await?;

        let status = response.status();
        let body = response.text().await?;

        if !status.is_success() {
            error!("Cloudflare API error (status {}): {}", status, body);
            return Err(Ddns6Error::CloudflareApi(format!(
                "Failed to create record: {} - {}",
                status, body
            )));
        }

        let create_response: CloudflareResponse<DnsRecord> =
            serde_json::from_str(&body).map_err(|e| {
                error!(
                    "Failed to parse Cloudflare response: {} - Body: {}",
                    e, body
                );
                Ddns6Error::CloudflareApi(format!("Failed to parse response: {}", e))
            })?;

        if !create_response.success {
            let error_msg = create_response
                .errors
                .iter()
                .map(|e| format!("{}: {}", e.code, e.message))
                .collect::<Vec<_>>()
                .join(", ");
            return Err(Ddns6Error::CloudflareApi(format!(
                "Cloudflare API returned errors: {}",
                error_msg
            )));
        }

        Ok(())
    }

    async fn update_record(
        &self,
        record_id: &str,
        hostname: &str,
        ipv6_address: Ipv6Addr,
    ) -> Result<()> {
        let url = format!(
            "https://api.cloudflare.com/client/v4/zones/{}/dns_records/{}",
            self.zone_id, record_id
        );

        let request = UpdateRecordRequest {
            record_type: "AAAA".to_string(),
            name: hostname.to_string(),
            content: ipv6_address.to_string(),
            ttl: self.ttl,
            proxied: false,
        };

        debug!("Updating AAAA record {}: {:?}", record_id, request);

        let response = self
            .client
            .put(&url)
            .bearer_auth(&self.api_token)
            .json(&request)
            .send()
            .await?;

        let status = response.status();
        let body = response.text().await?;

        if !status.is_success() {
            error!("Cloudflare API error (status {}): {}", status, body);
            return Err(Ddns6Error::CloudflareApi(format!(
                "Failed to update record: {} - {}",
                status, body
            )));
        }

        let update_response: CloudflareResponse<DnsRecord> =
            serde_json::from_str(&body).map_err(|e| {
                error!(
                    "Failed to parse Cloudflare response: {} - Body: {}",
                    e, body
                );
                Ddns6Error::CloudflareApi(format!("Failed to parse response: {}", e))
            })?;

        if !update_response.success {
            let error_msg = update_response
                .errors
                .iter()
                .map(|e| format!("{}: {}", e.code, e.message))
                .collect::<Vec<_>>()
                .join(", ");
            return Err(Ddns6Error::CloudflareApi(format!(
                "Cloudflare API returned errors: {}",
                error_msg
            )));
        }

        Ok(())
    }
}
