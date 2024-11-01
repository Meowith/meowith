use crate::error::std_response::NodeClientError;
use std::net::IpAddr;
use std::str::FromStr;

pub fn serialize_header(ip_addrs: Vec<IpAddr>) -> String {
    let mut header = String::new();
    let len = ip_addrs.len();
    let mut i = 0;
    for ip in ip_addrs {
        header.push_str(ip.to_string().as_str());
        i += 1;
        if i != len {
            header.push(',');
        }
    }
    header
}

pub fn deserialize_header(header: String) -> Result<Vec<IpAddr>, NodeClientError> {
    let mut addrs = Vec::new();

    for addr in header.split(",") {
        addrs.push(IpAddr::from_str(addr).map_err(|_| NodeClientError::InternalError)?);
    }

    Ok(addrs)
}

#[cfg(test)]
mod tests {
    use crate::autoconfigure::addr_header::{deserialize_header, serialize_header};
    use std::net::IpAddr;

    #[test]
    fn test_serialize_single() {
        let serialized_header = serialize_header(vec![IpAddr::from([1, 2, 3, 4])]);

        assert_eq!(serialized_header, "1.2.3.4");
    }

    #[test]
    fn test_serialize_multiple() {
        let serialized_header = serialize_header(vec![
            IpAddr::from([1, 2, 3, 4]),
            IpAddr::from([19, 22, 3, 4]),
            IpAddr::from([190, 21, 30, 4]),
        ]);

        assert_eq!(serialized_header, "1.2.3.4,19.22.3.4,190.21.30.4");
    }

    #[test]
    fn test_deserialize_single() {
        let deserialized_header = deserialize_header("1.2.3.4".to_string()).unwrap();

        assert_eq!(deserialized_header, vec![IpAddr::from([1, 2, 3, 4])]);
    }

    #[test]
    fn test_deserialize_multiple() {
        let deserialized_header =
            deserialize_header("1.2.3.4,2.3.4.5,190.22.5.1".to_string()).unwrap();

        assert_eq!(
            deserialized_header,
            vec![
                IpAddr::from([1, 2, 3, 4]),
                IpAddr::from([2, 3, 4, 5]),
                IpAddr::from([190, 22, 5, 1])
            ]
        );
    }
}
