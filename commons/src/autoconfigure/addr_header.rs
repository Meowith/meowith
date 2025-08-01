use crate::error::std_response::NodeClientError;
use std::net::IpAddr;
use std::str::FromStr;

pub fn serialize_header(ip_addrs: Vec<IpAddr>, domains: Vec<String>) -> String {
    let mut parts: Vec<String> = ip_addrs.into_iter().map(|ip| ip.to_string()).collect();
    parts.extend(domains);

    parts.join(",")
}

pub fn deserialize_header(header: String) -> Result<(Vec<IpAddr>, Vec<String>), NodeClientError> {
    let mut ip_addrs = Vec::new();
    let mut domains = Vec::new();

    for item in header.split(',') {
        if let Ok(ip) = IpAddr::from_str(item) {
            ip_addrs.push(ip);
        } else {
            domains.push(item.to_string());
        }
    }

    Ok((ip_addrs, domains))
}

#[cfg(test)]
mod tests {
    use crate::autoconfigure::addr_header::{deserialize_header, serialize_header};
    use std::net::IpAddr;

    #[test]
    fn test_serialize_single() {
        let serialized_header = serialize_header(vec![IpAddr::from([1, 2, 3, 4])], vec![]);

        assert_eq!(serialized_header, "1.2.3.4");
    }

    #[test]
    fn test_serialize_multiple() {
        let serialized_header = serialize_header(
            vec![
                IpAddr::from([1, 2, 3, 4]),
                IpAddr::from([19, 22, 3, 4]),
                IpAddr::from([190, 21, 30, 4]),
            ],
            vec![],
        );

        assert_eq!(serialized_header, "1.2.3.4,19.22.3.4,190.21.30.4");
    }

    #[test]
    fn test_serialize_with_domains() {
        let serialized_header = serialize_header(
            vec![IpAddr::from([1, 2, 3, 4])],
            vec!["example.com".to_string(), "test.org".to_string()],
        );

        assert_eq!(serialized_header, "1.2.3.4,example.com,test.org");
    }

    #[test]
    fn test_deserialize_single_ip() {
        let (ips, domains) = deserialize_header("1.2.3.4".to_string()).unwrap();

        assert_eq!(ips, vec![IpAddr::from([1, 2, 3, 4])]);
        assert!(domains.is_empty());
    }

    #[test]
    fn test_deserialize_ips_and_domains() {
        let (ips, domains) =
            deserialize_header("1.2.3.4,example.com,190.22.5.1,test.org".to_string()).unwrap();

        assert_eq!(
            ips,
            vec![IpAddr::from([1, 2, 3, 4]), IpAddr::from([190, 22, 5, 1])]
        );
        assert_eq!(domains, vec!["example.com", "test.org"]);
    }
}
