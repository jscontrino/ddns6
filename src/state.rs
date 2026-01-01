use std::collections::HashMap;
use std::net::Ipv6Addr;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Debug, Clone)]
pub struct HostState {
    pub ipv6_address: Ipv6Addr,
    #[allow(dead_code)]
    pub last_updated: std::time::SystemTime,
}

#[derive(Debug, Clone)]
pub struct StateCache {
    cache: Arc<RwLock<HashMap<String, HostState>>>,
}

impl StateCache {
    pub fn new() -> Self {
        Self {
            cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    #[allow(dead_code)]
    pub async fn get(&self, hostname: &str) -> Option<HostState> {
        let cache = self.cache.read().await;
        cache.get(hostname).cloned()
    }

    pub async fn has_changed(&self, hostname: &str, new_address: Ipv6Addr) -> bool {
        let cache = self.cache.read().await;
        match cache.get(hostname) {
            Some(state) => state.ipv6_address != new_address,
            None => true,
        }
    }

    pub async fn update(&self, hostname: String, ipv6_address: Ipv6Addr) {
        let mut cache = self.cache.write().await;
        cache.insert(
            hostname,
            HostState {
                ipv6_address,
                last_updated: std::time::SystemTime::now(),
            },
        );
    }

    #[allow(dead_code)]
    pub async fn remove(&self, hostname: &str) {
        let mut cache = self.cache.write().await;
        cache.remove(hostname);
    }

    #[allow(dead_code)]
    pub async fn list_all(&self) -> Vec<(String, HostState)> {
        let cache = self.cache.read().await;
        cache.iter().map(|(k, v)| (k.clone(), v.clone())).collect()
    }
}

impl Default for StateCache {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_state_cache() {
        let cache = StateCache::new();
        let hostname = "device1.example.com".to_string();
        let addr1 = "2001:db8::1".parse::<Ipv6Addr>().unwrap();
        let addr2 = "2001:db8::2".parse::<Ipv6Addr>().unwrap();

        assert!(cache.get(&hostname).await.is_none());

        assert!(cache.has_changed(&hostname, addr1).await);

        cache.update(hostname.clone(), addr1).await;

        let state = cache.get(&hostname).await.unwrap();
        assert_eq!(state.ipv6_address, addr1);

        assert!(!cache.has_changed(&hostname, addr1).await);

        assert!(cache.has_changed(&hostname, addr2).await);

        cache.update(hostname.clone(), addr2).await;
        let state = cache.get(&hostname).await.unwrap();
        assert_eq!(state.ipv6_address, addr2);
    }

    #[tokio::test]
    async fn test_remove() {
        let cache = StateCache::new();
        let hostname = "device1.example.com".to_string();
        let addr = "2001:db8::1".parse::<Ipv6Addr>().unwrap();

        cache.update(hostname.clone(), addr).await;
        assert!(cache.get(&hostname).await.is_some());

        cache.remove(&hostname).await;
        assert!(cache.get(&hostname).await.is_none());
    }

    #[tokio::test]
    async fn test_list_all() {
        let cache = StateCache::new();
        let addr1 = "2001:db8::1".parse::<Ipv6Addr>().unwrap();
        let addr2 = "2001:db8::2".parse::<Ipv6Addr>().unwrap();

        cache.update("device1.example.com".to_string(), addr1).await;
        cache.update("device2.example.com".to_string(), addr2).await;

        let all = cache.list_all().await;
        assert_eq!(all.len(), 2);
    }
}
