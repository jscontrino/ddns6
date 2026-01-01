use ipnet::Ipv6Net;
use std::net::Ipv6Addr;

use crate::error::{Ddns6Error, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ipv6Prefix {
    addr: Ipv6Addr,
    prefix_len: u8,
}

impl Ipv6Prefix {
    #[allow(dead_code)]
    pub fn new(addr: Ipv6Addr, prefix_len: u8) -> Self {
        Self { addr, prefix_len }
    }

    pub fn from_address(addr: Ipv6Addr, prefix_len: u8) -> Result<Self> {
        if prefix_len > 128 {
            return Err(Ddns6Error::Ipv6Parse(format!(
                "Invalid prefix length: {}",
                prefix_len
            )));
        }

        let network = Ipv6Net::new(addr, prefix_len)
            .map_err(|e| Ddns6Error::Ipv6Parse(format!("Failed to create network: {}", e)))?;

        Ok(Self {
            addr: network.network(),
            prefix_len,
        })
    }

    pub fn extract_from_address(addr: Ipv6Addr, default_prefix_len: u8) -> Result<Self> {
        Self::from_address(addr, default_prefix_len)
    }

    pub fn network(&self) -> Ipv6Addr {
        self.addr
    }

    pub fn prefix_len(&self) -> u8 {
        self.prefix_len
    }

    pub fn combine_with_interface_id(&self, interface_id: &str) -> Result<Ipv6Addr> {
        let iid_addr = parse_interface_id(interface_id)?;

        let prefix_bytes = self.addr.octets();
        let iid_bytes = iid_addr.octets();

        let boundary = (self.prefix_len / 8) as usize;
        let mut result_bytes = [0u8; 16];

        result_bytes[..boundary].copy_from_slice(&prefix_bytes[..boundary]);

        if !self.prefix_len.is_multiple_of(8) {
            let mask_bits = self.prefix_len % 8;
            let mask = (!0u8) << (8 - mask_bits);
            result_bytes[boundary] =
                (prefix_bytes[boundary] & mask) | (iid_bytes[boundary] & !mask);
            result_bytes[boundary + 1..].copy_from_slice(&iid_bytes[boundary + 1..]);
        } else {
            result_bytes[boundary..].copy_from_slice(&iid_bytes[boundary..]);
        }

        Ok(Ipv6Addr::from(result_bytes))
    }
}

fn parse_interface_id(iid: &str) -> Result<Ipv6Addr> {
    if let Ok(addr) = iid.parse::<Ipv6Addr>() {
        return Ok(addr);
    }

    let test_addr = format!("2001:db8::{}", iid);
    if let Ok(addr) = test_addr.parse::<Ipv6Addr>() {
        return Ok(addr);
    }

    let test_addr = format!("::ffff:{}", iid);
    if let Ok(addr) = test_addr.parse::<Ipv6Addr>() {
        return Ok(addr);
    }

    Err(Ddns6Error::InvalidInterfaceId(format!(
        "Could not parse interface ID: {}",
        iid
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_prefix() {
        let addr = "2001:db8:1234:5678::1".parse::<Ipv6Addr>().unwrap();
        let prefix = Ipv6Prefix::extract_from_address(addr, 64).unwrap();

        assert_eq!(prefix.prefix_len(), 64);
        assert_eq!(
            prefix.network(),
            "2001:db8:1234:5678::".parse::<Ipv6Addr>().unwrap()
        );
    }

    #[test]
    fn test_combine_with_interface_id() {
        let addr = "2001:db8:1234:5678::1".parse::<Ipv6Addr>().unwrap();
        let prefix = Ipv6Prefix::extract_from_address(addr, 64).unwrap();

        let result = prefix.combine_with_interface_id("::1").unwrap();
        assert_eq!(result, "2001:db8:1234:5678::1".parse::<Ipv6Addr>().unwrap());

        let result = prefix.combine_with_interface_id("::2").unwrap();
        assert_eq!(result, "2001:db8:1234:5678::2".parse::<Ipv6Addr>().unwrap());

        let result = prefix
            .combine_with_interface_id("::a1b2:c3d4:e5f6:7890")
            .unwrap();
        assert_eq!(
            result,
            "2001:db8:1234:5678:a1b2:c3d4:e5f6:7890"
                .parse::<Ipv6Addr>()
                .unwrap()
        );
    }

    #[test]
    fn test_parse_interface_id() {
        assert!(parse_interface_id("::1").is_ok());
        assert!(parse_interface_id("::2").is_ok());
        assert!(parse_interface_id("1").is_ok());
        assert!(parse_interface_id("a1b2:c3d4:e5f6:7890").is_ok());
    }

    #[test]
    fn test_prefix_56() {
        let addr = "2001:db8:1234:5600::1".parse::<Ipv6Addr>().unwrap();
        let prefix = Ipv6Prefix::extract_from_address(addr, 56).unwrap();

        assert_eq!(prefix.prefix_len(), 56);

        let result = prefix.combine_with_interface_id("::1:0:0:1").unwrap();
        assert_eq!(result.to_string(), "2001:db8:1234:5600:1::1");
    }

    #[test]
    fn test_prefix_48() {
        let addr = "2001:db8:1200::1".parse::<Ipv6Addr>().unwrap();
        let prefix = Ipv6Prefix::extract_from_address(addr, 48).unwrap();

        assert_eq!(prefix.prefix_len(), 48);

        let result = prefix.combine_with_interface_id("::1:2:3:4").unwrap();
        assert_eq!(result.to_string(), "2001:db8:1200:0:1:2:3:4");
    }

    #[test]
    fn test_prefix_length_validation() {
        let addr = "2001:db8::1".parse::<Ipv6Addr>().unwrap();

        assert!(Ipv6Prefix::from_address(addr, 0).is_ok());
        assert!(Ipv6Prefix::from_address(addr, 64).is_ok());
        assert!(Ipv6Prefix::from_address(addr, 128).is_ok());
        assert!(Ipv6Prefix::from_address(addr, 129).is_err());
    }

    #[test]
    fn test_ipv6_prefix_new() {
        let addr = "2001:db8::".parse::<Ipv6Addr>().unwrap();
        let prefix = Ipv6Prefix::new(addr, 64);

        assert_eq!(prefix.network(), addr);
        assert_eq!(prefix.prefix_len(), 64);
    }

    #[test]
    fn test_parse_interface_id_formats() {
        assert!(parse_interface_id("::1").is_ok());
        assert!(parse_interface_id("::ffff:192.168.1.1").is_ok());
        assert!(parse_interface_id("fe80::1").is_ok());
        assert!(parse_interface_id("1234:5678:90ab:cdef").is_ok());
        assert!(parse_interface_id("1").is_ok());
        assert!(parse_interface_id("ff:ff:ff:ff").is_ok());
    }

    #[test]
    fn test_parse_interface_id_invalid() {
        assert!(parse_interface_id("not-an-address").is_err());
        assert!(parse_interface_id("999.999.999.999").is_err());
        assert!(parse_interface_id("gggg::1").is_err());
    }

    #[test]
    fn test_combine_with_various_prefix_lengths() {
        let addr32 = "2001:db8::".parse::<Ipv6Addr>().unwrap();
        let prefix32 = Ipv6Prefix::from_address(addr32, 32).unwrap();
        let result = prefix32
            .combine_with_interface_id("::1234:5678:90ab:cdef")
            .unwrap();
        assert_eq!(result.to_string(), "2001:db8::1234:5678:90ab:cdef");

        let addr80 = "2001:db8:1234:5678:90ab::".parse::<Ipv6Addr>().unwrap();
        let prefix80 = Ipv6Prefix::from_address(addr80, 80).unwrap();
        let result = prefix80.combine_with_interface_id("::cdef").unwrap();
        assert!(result.to_string().starts_with("2001:db8:1234:5678:90ab"));
    }

    #[test]
    fn test_combine_preserves_prefix() {
        let addr = "2001:db8:abcd:ef01::".parse::<Ipv6Addr>().unwrap();
        let prefix = Ipv6Prefix::from_address(addr, 64).unwrap();

        let result1 = prefix.combine_with_interface_id("::1").unwrap();
        let result2 = prefix.combine_with_interface_id("::2").unwrap();

        assert!(result1.to_string().starts_with("2001:db8:abcd:ef01"));
        assert!(result2.to_string().starts_with("2001:db8:abcd:ef01"));
        assert_ne!(result1, result2);
    }

    #[test]
    fn test_edge_case_all_zeros() {
        let addr = "::".parse::<Ipv6Addr>().unwrap();
        let prefix = Ipv6Prefix::extract_from_address(addr, 64).unwrap();
        let result = prefix.combine_with_interface_id("::1").unwrap();
        assert_eq!(result.to_string(), "::1");
    }

    #[test]
    fn test_edge_case_all_ones() {
        let addr = "ffff:ffff:ffff:ffff::".parse::<Ipv6Addr>().unwrap();
        let prefix = Ipv6Prefix::extract_from_address(addr, 64).unwrap();
        let result = prefix.combine_with_interface_id("::1").unwrap();
        assert!(result.to_string().starts_with("ffff:ffff:ffff:ffff"));
    }
}
