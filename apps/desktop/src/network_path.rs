use if_addrs::Interface;
use std::net::{IpAddr, Ipv4Addr};

const WILDCARD_QUIC_LISTEN: &str = "/ip4/0.0.0.0/udp/0/quic-v1";

#[derive(Debug, Clone, PartialEq, Eq)]
struct LocalIpv4Interface {
    name: String,
    address: Ipv4Addr,
    is_operational: bool,
    is_point_to_point: bool,
}

pub(crate) fn synchronization_listen_addresses() -> Vec<String> {
    // Interface selection is only an escape hatch for broken default routes.
    // Enumeration failure must preserve the previous wildcard behavior.
    let interfaces = if_addrs::get_if_addrs()
        .unwrap_or_default()
        .into_iter()
        .filter_map(local_ipv4_interface)
        .collect::<Vec<_>>();
    synchronization_listen_addresses_from(interfaces)
}

fn local_ipv4_interface(interface: Interface) -> Option<LocalIpv4Interface> {
    let IpAddr::V4(address) = interface.ip() else {
        return None;
    };
    let is_operational = interface.is_oper_up();
    let is_point_to_point = interface.is_p2p();
    Some(LocalIpv4Interface {
        name: interface.name,
        address,
        is_operational,
        is_point_to_point,
    })
}

fn synchronization_listen_addresses_from(mut interfaces: Vec<LocalIpv4Interface>) -> Vec<String> {
    interfaces.retain(|interface| interface.address.is_private());
    interfaces.sort_by_key(interface_preference);

    let mut addresses = interfaces
        .first()
        .map(|interface| vec![format!("/ip4/{}/udp/0/quic-v1", interface.address)])
        .unwrap_or_default();
    addresses.push(WILDCARD_QUIC_LISTEN.to_owned());
    addresses
}

fn interface_preference(interface: &LocalIpv4Interface) -> (bool, bool, bool, String, Ipv4Addr) {
    (
        !interface.is_operational,
        interface.is_point_to_point,
        is_known_virtual_interface(&interface.name),
        interface.name.to_ascii_lowercase(),
        interface.address,
    )
}

fn is_known_virtual_interface(name: &str) -> bool {
    let name = name.to_ascii_lowercase();
    [
        "utun",
        "tun",
        "tap",
        "tailscale",
        "docker",
        "veth",
        "virbr",
        "vmnet",
        "vbox",
        "hyper-v",
        "vethernet",
        "bridge",
        "br-",
    ]
    .iter()
    .any(|marker| name.contains(marker))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn interface(
        name: &str,
        address: [u8; 4],
        is_operational: bool,
        is_point_to_point: bool,
    ) -> LocalIpv4Interface {
        LocalIpv4Interface {
            name: name.to_owned(),
            address: Ipv4Addr::from(address),
            is_operational,
            is_point_to_point,
        }
    }

    #[test]
    fn physical_private_address_precedes_tunnel_and_wildcard() {
        let addresses = synchronization_listen_addresses_from(vec![
            interface("utun1024", [198, 18, 0, 1], true, true),
            interface("en0", [192, 168, 1, 39], true, false),
        ]);

        assert_eq!(
            addresses,
            ["/ip4/192.168.1.39/udp/0/quic-v1", WILDCARD_QUIC_LISTEN]
        );
    }

    #[test]
    fn physical_interface_precedes_private_virtual_adapter() {
        let addresses = synchronization_listen_addresses_from(vec![
            interface("docker0", [172, 17, 0, 1], true, false),
            interface("Ethernet", [10, 0, 0, 8], true, false),
        ]);

        assert_eq!(addresses[0], "/ip4/10.0.0.8/udp/0/quic-v1");
    }

    #[test]
    fn wildcard_is_used_when_no_private_ipv4_exists() {
        let addresses = synchronization_listen_addresses_from(vec![
            interface("en0", [203, 0, 113, 8], true, false),
            interface("utun7", [198, 18, 0, 1], true, true),
        ]);

        assert_eq!(addresses, [WILDCARD_QUIC_LISTEN]);
    }
}
