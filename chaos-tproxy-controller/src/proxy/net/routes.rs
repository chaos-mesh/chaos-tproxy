use anyhow::{anyhow, Context, Result};
use futures_util::future::join_all;
use iproute2_rs::ip::iproute::{del_routes, get_routes, Action, IPRoute};
use pnet::ipnetwork::Ipv4Network;
use rtnetlink::packet::route::{NextHopBuffer, Nla};
use rtnetlink::packet::{RouteMessage, RTN_UNICAST, RTPROT_KERNEL, RT_SCOPE_LINK, RT_TABLE_MAIN};
use rtnetlink::{Handle, IpVersion};

pub async fn get_routes_noblock(handle: &Handle) -> Result<Vec<RouteMessage>> {
    let routes = get_routes(handle, IpVersion::V4).await?;
    Ok(routes
        .into_iter()
        .filter(|route| route_table(route) != 255)
        .collect())
}

pub async fn del_routes_noblock(handle: &Handle, msgs: Vec<RouteMessage>) -> Result<()> {
    let results = join_all(msgs.into_iter().map(|msg| del_routes(handle, msg))).await;
    match results
        .into_iter()
        .filter_map(Result::err)
        .reduce(|accum, item| anyhow!("{} \n {}", accum, item))
    {
        Some(e) => Err(e),
        None => Ok(()),
    }
}

pub(super) fn route_uses_device(route: &RouteMessage, index: u32) -> bool {
    route.nlas.iter().any(|nla| match nla {
        Nla::Oif(oif) => *oif == index,
        Nla::MultiPath(bytes) => {
            let mut remaining = bytes.as_slice();
            while !remaining.is_empty() {
                let Ok(next_hop) = NextHopBuffer::new_checked(remaining) else {
                    return false;
                };
                let length = next_hop.length() as usize;
                if length < 8 {
                    return false;
                }
                if next_hop.interface_id() == index {
                    return true;
                }
                let aligned_length = (length + 3) & !3;
                let Some(rest) = remaining.get(aligned_length..) else {
                    return false;
                };
                remaining = rest;
            }
            false
        }
        _ => false,
    })
}

fn route_table(route: &RouteMessage) -> u32 {
    route
        .nlas
        .iter()
        .find_map(|nla| match nla {
            Nla::Table(table) => Some(*table),
            _ => None,
        })
        .unwrap_or(route.header.table as u32)
}

#[derive(Debug, PartialEq, Eq)]
struct RouteSlot {
    family: u8,
    table: u32,
    destination_prefix: u8,
    destination: Vec<u8>,
    source_prefix: u8,
    source: Vec<u8>,
    tos: u8,
}

impl From<&RouteMessage> for RouteSlot {
    fn from(route: &RouteMessage) -> Self {
        Self {
            family: route.header.address_family,
            table: route_table(route),
            destination_prefix: route.header.destination_prefix_length,
            destination: route
                .nlas
                .iter()
                .find_map(|nla| match nla {
                    Nla::Destination(address) if route.header.destination_prefix_length > 0 => {
                        Some(address.clone())
                    }
                    _ => None,
                })
                .unwrap_or_default(),
            source_prefix: route.header.source_prefix_length,
            source: route
                .nlas
                .iter()
                .find_map(|nla| match nla {
                    Nla::Source(address) if route.header.source_prefix_length > 0 => {
                        Some(address.clone())
                    }
                    _ => None,
                })
                .unwrap_or_default(),
            tos: route.header.tos,
        }
    }
}

fn same_route(left: &RouteMessage, right: &RouteMessage) -> bool {
    fn attributes(route: &RouteMessage) -> Vec<&Nla> {
        route
            .nlas
            .iter()
            .filter(|nla| match nla {
                Nla::Table(_) | Nla::CacheInfo(_) | Nla::Pad(_) | Nla::Priority(0) => false,
                Nla::Destination(_) if route.header.destination_prefix_length == 0 => false,
                Nla::Source(_) if route.header.source_prefix_length == 0 => false,
                _ => true,
            })
            .collect()
    }
    let left_attributes = attributes(left);
    let right_attributes = attributes(right);
    RouteSlot::from(left) == RouteSlot::from(right)
        && left.header.protocol == right.header.protocol
        && left.header.scope == right.header.scope
        && left.header.kind == right.header.kind
        && left_attributes.len() == right_attributes.len()
        && left_attributes
            .iter()
            .all(|nla| right_attributes.contains(nla))
}

fn same_route_value(left: &RouteMessage, right: &RouteMessage) -> bool {
    left.header.flags == right.header.flags && same_route(left, right)
}

pub(super) fn retain_displaced_routes(originals: &mut Vec<RouteMessage>, current: &[RouteMessage]) {
    originals.retain(|original| !current.iter().any(|route| same_route(original, route)));
}

pub(super) fn generated_address_route(
    before: &[RouteMessage],
    address: Ipv4Network,
    device: u32,
) -> Option<RouteMessage> {
    let mut connected = RouteMessage::default();
    connected.header.address_family = libc::AF_INET as u8;
    connected.header.destination_prefix_length = address.prefix();
    connected.header.table = RT_TABLE_MAIN;
    connected.header.protocol = RTPROT_KERNEL;
    connected.header.scope = RT_SCOPE_LINK;
    connected.header.kind = RTN_UNICAST;
    connected.nlas = vec![
        Nla::Destination(address.network().octets().to_vec()),
        Nla::Oif(device),
        Nla::PrefSource(address.ip().octets().to_vec()),
    ];
    if before.iter().any(|route| same_route(route, &connected)) {
        None
    } else {
        Some(connected)
    }
}

fn address_route_to_remove(
    pending: &mut Option<RouteMessage>,
    current: &[RouteMessage],
) -> Option<RouteMessage> {
    let owned = pending.as_ref()?;
    let route = current
        .iter()
        .find(|route| same_route(route, owned) && route.header.flags == owned.header.flags)
        .cloned();
    if route.is_none() {
        *pending = None;
    }
    route
}

pub(super) async fn remove_generated_address_route(
    handle: &Handle,
    pending: &mut Option<RouteMessage>,
) -> Result<()> {
    if pending.is_none() {
        return Ok(());
    }
    let current = get_routes_noblock(handle).await?;
    if let Some(route) = address_route_to_remove(pending, &current) {
        del_routes(handle, route).await?;
        *pending = None;
    }
    Ok(())
}

#[derive(Debug, PartialEq, Eq)]
enum RestorationDecision {
    Add,
    Present,
    Superseded,
}

fn restoration_decision(
    original: &RouteMessage,
    current: &[RouteMessage],
    baseline: &[RouteMessage],
    restored: &[RouteMessage],
) -> RestorationDecision {
    if current
        .iter()
        .any(|route| same_route_value(original, route))
    {
        return RestorationDecision::Present;
    }
    if current.iter().any(|route| {
        RouteSlot::from(original) == RouteSlot::from(route)
            && !baseline
                .iter()
                .any(|before| same_route_value(before, route))
            && !restored.iter().any(|owned| same_route_value(owned, route))
    }) {
        RestorationDecision::Superseded
    } else {
        RestorationDecision::Add
    }
}

fn record_restored_route(restored: &mut Vec<RouteMessage>, route: RouteMessage) {
    if !restored.iter().any(|owned| same_route_value(owned, &route)) {
        restored.push(route);
    }
}

fn restoration_plan(
    displaced: &[RouteMessage],
    current: &[RouteMessage],
    baseline: &[RouteMessage],
    restored: &mut Vec<RouteMessage>,
) -> Vec<RouteMessage> {
    for original in displaced {
        if restoration_decision(original, current, baseline, restored)
            == RestorationDecision::Present
        {
            record_restored_route(restored, original.clone());
        }
    }
    let mut plan: Vec<_> = displaced
        .iter()
        .filter(|original| {
            restoration_decision(original, current, baseline, restored) == RestorationDecision::Add
        })
        .cloned()
        .collect();
    plan.sort_by_key(|route| {
        route
            .nlas
            .iter()
            .any(|nla| matches!(nla, Nla::Gateway(_) | Nla::MultiPath(_)))
    });
    plan
}

pub async fn restore_routes(
    handle: &mut Handle,
    baseline: &[RouteMessage],
    displaced: &mut Vec<RouteMessage>,
    restored: &mut Vec<RouteMessage>,
) -> Result<()> {
    if displaced.is_empty() {
        return Ok(());
    }
    let current = get_routes_noblock(handle).await?;
    let plan = restoration_plan(displaced, &current, baseline, restored);
    let mut failed = Vec::new();
    let mut errors = Vec::new();
    for route in plan {
        if let Err(error) = (IPRoute {
            action: Action::Add,
            msg: route.clone(),
        })
        .execute(handle)
        .await
        {
            let exists = matches!(
                error.downcast_ref::<rtnetlink::Error>(),
                Some(rtnetlink::Error::NetlinkError(message)) if message.code == -libc::EEXIST
            );
            if exists {
                match get_routes_noblock(handle).await {
                    Ok(routes) => match restoration_decision(&route, &routes, baseline, restored) {
                        RestorationDecision::Present => {
                            record_restored_route(restored, route);
                            continue;
                        }
                        RestorationDecision::Superseded => continue,
                        RestorationDecision::Add => {}
                    },
                    Err(recheck) => errors.push(format!("recheck route conflict: {recheck:#}")),
                }
            }
            errors.push(format!("restore route {route:?}: {error:#}"));
            failed.push(route);
        } else {
            record_restored_route(restored, route);
        }
    }
    *displaced = failed;
    if errors.is_empty() {
        Ok(())
    } else {
        Err(anyhow!(errors.join("; "))).context("restore displaced routes")
    }
}

pub async fn load_routes(handle: &mut Handle, mut msgs: Vec<RouteMessage>) -> Result<()> {
    let baseline = msgs.clone();
    restore_routes(handle, &baseline, &mut msgs, &mut Vec::new()).await
}

#[cfg(test)]
mod test {
    use std::slice::from_ref;

    use rtnetlink::new_connection;
    use tokio::spawn;

    use super::*;

    fn route(destination: [u8; 4], prefix: u8, device: u32, metric: u32) -> RouteMessage {
        let mut route = RouteMessage::default();
        route.header.address_family = libc::AF_INET as u8;
        route.header.table = 254;
        route.header.kind = 1;
        route.header.destination_prefix_length = prefix;
        route.nlas = vec![
            Nla::Destination(destination.to_vec()),
            Nla::Oif(device),
            Nla::Priority(metric),
        ];
        route
    }

    fn connected_route() -> RouteMessage {
        let mut connected = route([192, 0, 2, 0], 24, 2, 0);
        connected.header.protocol = 2;
        connected.header.scope = 253;
        connected
            .nlas
            .retain(|nla| !matches!(nla, Nla::Priority(_)));
        connected.nlas.push(Nla::PrefSource(vec![192, 0, 2, 10]));
        connected
    }

    fn multipath_route(next_hops: &[(u32, u8)]) -> RouteMessage {
        let mut route = route([198, 51, 100, 0], 24, 2, 0);
        route.nlas.retain(|nla| !matches!(nla, Nla::Oif(_)));
        let mut bytes = Vec::new();
        for (device, weight) in next_hops {
            bytes.extend_from_slice(&16u16.to_ne_bytes());
            bytes.extend_from_slice(&[0, weight - 1]);
            bytes.extend_from_slice(&device.to_ne_bytes());
            bytes.extend_from_slice(&8u16.to_ne_bytes());
            bytes.extend_from_slice(&5u16.to_ne_bytes());
            bytes.extend_from_slice(&[192, 0, 2, 1]);
        }
        route.nlas.push(Nla::MultiPath(bytes));
        route
    }

    #[test]
    fn generated_connected_route_is_removed_before_restoring_original_alternatives() {
        let connected = connected_route();
        for metric in [0, 111] {
            let alternative = route([192, 0, 2, 0], 24, 2, metric);
            for originals in [
                vec![alternative.clone()],
                vec![connected.clone(), alternative.clone()],
            ] {
                let mut pending = generated_address_route(&[], "192.0.2.10/24".parse().unwrap(), 2);
                assert_eq!(pending, Some(connected.clone()));
                let mut current = vec![connected.clone()];
                let removal = address_route_to_remove(&mut pending, &current);
                assert_eq!(removal, Some(connected.clone()));
                current.retain(|route| Some(route) != removal.as_ref());
                assert_eq!(
                    restoration_plan(&originals, &current, &originals, &mut Vec::new()),
                    originals
                );
            }
        }
    }

    #[test]
    fn address_restoration_does_not_claim_a_preexisting_route() {
        let connected = connected_route();
        let address = "192.0.2.10/24".parse().unwrap();
        let mut recreated = connected.clone();
        recreated.header.table = 0;
        recreated.nlas.push(Nla::Table(254));
        recreated.nlas.push(Nla::Priority(0));
        recreated.nlas.push(Nla::CacheInfo(vec![1, 2, 3]));
        recreated.nlas.reverse();
        let mut pending = generated_address_route(&[], address, 2);
        assert_eq!(
            address_route_to_remove(&mut pending, from_ref(&recreated)),
            Some(recreated.clone())
        );
        assert_eq!(generated_address_route(&[recreated], address, 2), None);
        assert_eq!(
            restoration_plan(
                from_ref(&connected),
                from_ref(&connected),
                from_ref(&connected),
                &mut Vec::new(),
            ),
            Vec::<RouteMessage>::new()
        );
    }

    #[test]
    fn foreign_connected_alternative_survives_generated_route_removal() {
        let connected = connected_route();
        let alternative = route([192, 0, 2, 0], 24, 2, 111);
        let foreign = route([192, 0, 2, 0], 24, 2, 222);
        let mut pending = generated_address_route(&[], "192.0.2.10/24".parse().unwrap(), 2);
        let mut current = vec![connected.clone(), foreign.clone()];
        let removal = address_route_to_remove(&mut pending, &current);
        assert_eq!(removal, Some(connected));
        current.retain(|route| Some(route) != removal.as_ref());
        assert_eq!(current, vec![foreign]);
        assert_eq!(
            restoration_plan(
                from_ref(&alternative),
                &current,
                from_ref(&alternative),
                &mut Vec::new(),
            ),
            Vec::<RouteMessage>::new()
        );
        assert_eq!(
            restoration_plan(
                from_ref(&alternative),
                &[],
                from_ref(&alternative),
                &mut Vec::new(),
            ),
            vec![alternative]
        );
    }

    #[test]
    fn address_route_removal_requires_the_exact_generated_route() {
        let connected = connected_route();
        let mut metric = connected.clone();
        metric.nlas.push(Nla::Priority(111));
        let mut gateway = connected.clone();
        gateway.nlas.push(Nla::Gateway(vec![192, 0, 2, 1]));
        let mut table = connected.clone();
        table.nlas.push(Nla::Table(1000));
        let mut source = connected.clone();
        source.nlas.retain(|nla| !matches!(nla, Nla::PrefSource(_)));
        source.nlas.push(Nla::PrefSource(vec![192, 0, 2, 20]));
        let mut protocol = connected.clone();
        protocol.header.protocol = 4;
        let mut scope = connected.clone();
        scope.header.scope = 0;
        let mut kind = connected.clone();
        kind.header.kind = 6;
        let mut device = connected.clone();
        device.nlas.retain(|nla| !matches!(nla, Nla::Oif(_)));
        device.nlas.push(Nla::Oif(7));
        let mut pending = Some(connected.clone());
        assert_eq!(
            address_route_to_remove(&mut pending, from_ref(&connected)),
            Some(connected.clone())
        );
        for foreign in [
            metric, gateway, table, source, protocol, scope, kind, device,
        ] {
            let mut pending = Some(connected.clone());
            assert_eq!(address_route_to_remove(&mut pending, &[foreign]), None);
            assert_eq!(pending, None);
        }
    }

    #[test]
    fn address_route_removal_plan_can_be_retried_until_the_route_is_gone() {
        let connected = connected_route();
        let mut pending = generated_address_route(&[], "192.0.2.10/24".parse().unwrap(), 2);
        for _ in 0..2 {
            assert_eq!(
                address_route_to_remove(&mut pending, from_ref(&connected)),
                Some(connected.clone())
            );
            assert_eq!(pending, Some(connected.clone()));
        }
        assert_eq!(address_route_to_remove(&mut pending, &[]), None);
        assert_eq!(pending, None);
        assert_eq!(address_route_to_remove(&mut pending, &[connected]), None);
    }

    #[test]
    fn multipath_capture_restores_only_displaced_routes_using_the_original_device() {
        let original = multipath_route(&[(2, 1), (2, 2)]);
        let surviving = multipath_route(&[(7, 1), (2, 2)]);
        let unrelated = multipath_route(&[(7, 1), (7, 2)]);
        let mut displaced: Vec<_> = vec![original.clone(), surviving.clone(), unrelated.clone()]
            .into_iter()
            .filter(|route| route_uses_device(route, 2))
            .collect();
        assert_eq!(displaced, vec![original.clone(), surviving.clone()]);
        retain_displaced_routes(&mut displaced, &[surviving, unrelated]);
        assert_eq!(
            restoration_plan(&displaced, &[], &displaced, &mut Vec::new()),
            vec![original]
        );
    }

    #[test]
    fn connected_route_is_restored_before_multipath_gateways() {
        let multipath = multipath_route(&[(2, 1), (2, 2)]);
        let connected = connected_route();
        let baseline = vec![multipath.clone(), connected.clone()];
        assert_eq!(
            restoration_plan(&baseline, &[], &baseline, &mut Vec::new()),
            vec![connected, multipath]
        );
    }

    #[test]
    fn multipath_device_check_rejects_truncated_and_zero_length_hops() {
        let valid = multipath_route(&[(7, 1), (2, 2)]);
        assert!(route_uses_device(&valid, 2));
        for bytes in [Vec::new(), vec![0; 8], vec![16, 0, 0, 0, 2, 0, 0, 0]] {
            let mut invalid = route([198, 51, 100, 0], 24, 7, 0);
            invalid.nlas.push(Nla::MultiPath(bytes));
            assert!(!route_uses_device(&invalid, 2));
        }
    }

    #[test]
    fn missing_original_alternatives_are_restored_together() {
        let first = route([0; 4], 0, 2, 0);
        let second = route([0; 4], 0, 2, 100);
        let unrelated = route([203, 0, 113, 0], 24, 3, 0);
        let baseline = vec![first.clone(), second.clone(), unrelated.clone()];
        assert_eq!(
            restoration_plan(
                &[first.clone(), second.clone()],
                &[unrelated],
                &baseline,
                &mut Vec::new(),
            ),
            vec![first, second]
        );
    }

    #[test]
    fn unchanged_backup_allows_restoration_without_restoring_unrelated_baseline_routes() {
        let original = route([0; 4], 0, 2, 0);
        let backup = route([0; 4], 0, 7, 500);
        let unrelated = route([203, 0, 113, 0], 24, 3, 0);
        let baseline = vec![original.clone(), backup.clone(), unrelated];
        let mut current = backup;
        current.header.table = 0;
        current.nlas.push(Nla::Table(254));
        current.nlas.push(Nla::CacheInfo(vec![1, 2, 3]));
        current.nlas.push(Nla::Pad(vec![0]));
        current.nlas.reverse();
        assert_eq!(
            restoration_plan(from_ref(&original), &[current], &baseline, &mut Vec::new()),
            vec![original]
        );
    }

    #[test]
    fn changed_backup_and_new_default_win_over_missing_originals() {
        let original = route([0; 4], 0, 2, 0);
        let backup = route([0; 4], 0, 7, 500);
        let baseline = vec![original.clone(), backup.clone()];
        let metric = route([0; 4], 0, 7, 600);
        let device = route([0; 4], 0, 8, 500);
        let mut gateway = backup.clone();
        gateway.nlas.push(Nla::Gateway(vec![192, 0, 2, 8]));
        let mut protocol = backup.clone();
        protocol.header.protocol = 4;
        let mut scope = backup.clone();
        scope.header.scope = 253;
        let mut kind = backup.clone();
        kind.header.kind = 6;
        for changed in [metric, device, gateway, protocol, scope, kind] {
            assert_eq!(
                restoration_decision(&original, &[changed], &baseline, &[]),
                RestorationDecision::Superseded
            );
        }
        let new_default = route([0; 4], 0, 2, 222);
        assert_eq!(
            restoration_plan(
                from_ref(&original),
                &[backup.clone(), new_default],
                &baseline,
                &mut Vec::new(),
            ),
            Vec::<RouteMessage>::new()
        );
        assert_eq!(
            restoration_plan(from_ref(&original), &[backup], &baseline, &mut Vec::new()),
            vec![original]
        );
    }

    #[test]
    fn exact_presence_records_one_receipt_even_with_a_new_same_slot_route() {
        let original = route([0; 4], 0, 2, 0);
        let replacement = route([0; 4], 0, 7, 222);
        let mut equivalent = original.clone();
        equivalent.header.table = 0;
        equivalent.nlas = vec![Nla::Table(254), Nla::Oif(2), Nla::CacheInfo(vec![1, 2, 3])];
        let current = vec![equivalent, replacement];
        let mut restored = Vec::new();
        for _ in 0..2 {
            assert_eq!(
                restoration_plan(
                    from_ref(&original),
                    &current,
                    from_ref(&original),
                    &mut restored,
                ),
                Vec::<RouteMessage>::new()
            );
            assert_eq!(restored, vec![original.clone()]);
        }
        assert_eq!(
            restoration_plan(from_ref(&original), &[], from_ref(&original), &mut restored),
            vec![original]
        );
    }

    #[test]
    fn late_captured_present_routes_are_recorded_before_missing_alternatives_are_planned() {
        let pending = route([0; 4], 0, 2, 0);
        let late = route([0; 4], 0, 2, 100);
        for displaced in [
            vec![pending.clone(), late.clone()],
            vec![late.clone(), pending.clone()],
        ] {
            let mut restored = Vec::new();
            assert_eq!(
                restoration_plan(
                    &displaced,
                    from_ref(&late),
                    from_ref(&pending),
                    &mut restored,
                ),
                vec![pending.clone()]
            );
            assert_eq!(restored, vec![late.clone()]);
            assert_eq!(
                restoration_plan(
                    from_ref(&pending),
                    from_ref(&late),
                    from_ref(&pending),
                    &mut restored,
                ),
                vec![pending.clone()]
            );
            assert_eq!(restored, vec![late.clone()]);
        }
    }

    #[test]
    fn retry_keeps_the_full_baseline_after_a_partial_restore() {
        let original = route([0; 4], 0, 2, 0);
        let pending = route([0; 4], 0, 2, 100);
        let backup = route([0; 4], 0, 7, 500);
        let baseline = vec![original.clone(), pending.clone(), backup.clone()];
        let mut restored = vec![original.clone()];
        assert_eq!(
            restoration_plan(
                from_ref(&pending),
                &[original, backup],
                &baseline,
                &mut restored,
            ),
            vec![pending]
        );
    }

    #[test]
    fn represented_header_flags_and_multipath_changes_are_new_route_values() {
        let original = route([0; 4], 0, 2, 0);
        let mut flagged = original.clone();
        flagged.header.flags = !flagged.header.flags;
        assert!(same_route(&original, &flagged));
        assert!(!same_route_value(&original, &flagged));
        assert_eq!(
            restoration_decision(&original, from_ref(&flagged), from_ref(&original), &[]),
            RestorationDecision::Superseded
        );
        assert_eq!(
            restoration_decision(&original, &[flagged], &[], from_ref(&original)),
            RestorationDecision::Superseded
        );

        let original = multipath_route(&[(2, 1), (2, 2)]);
        let backup = multipath_route(&[(7, 1), (7, 2)]);
        let baseline = vec![original.clone(), backup.clone()];
        assert_eq!(
            restoration_decision(&original, &[backup], &baseline, &[]),
            RestorationDecision::Add
        );
        let changed = multipath_route(&[(7, 1), (7, 3)]);
        assert_eq!(
            restoration_decision(&original, &[changed], &baseline, &[]),
            RestorationDecision::Superseded
        );
    }

    #[test]
    fn conflict_recheck_requires_exact_presence_or_a_new_replacement() {
        let original = route([0; 4], 0, 2, 0);
        let backup = route([0; 4], 0, 7, 500);
        let baseline = vec![original.clone(), backup.clone()];
        assert_eq!(
            restoration_decision(&original, from_ref(&backup), &baseline, &[]),
            RestorationDecision::Add
        );
        assert_eq!(
            restoration_decision(
                &original,
                &[backup.clone(), original.clone()],
                &baseline,
                &[]
            ),
            RestorationDecision::Present
        );
        let replacement = route([0; 4], 0, 7, 222);
        assert_eq!(
            restoration_decision(&original, &[backup, replacement], &baseline, &[]),
            RestorationDecision::Superseded
        );
    }

    #[test]
    fn cleanup_retry_does_not_confuse_its_own_restored_alternative_with_a_foreign_route() {
        let restored = route([0; 4], 0, 2, 0);
        let pending = route([0; 4], 0, 2, 100);
        assert_eq!(
            restoration_plan(
                from_ref(&pending),
                from_ref(&restored),
                from_ref(&pending),
                &mut vec![restored.clone()],
            ),
            vec![pending.clone()]
        );
        let replacement = route([0; 4], 0, 2, 222);
        assert_eq!(
            restoration_plan(
                from_ref(&pending),
                &[replacement],
                from_ref(&pending),
                &mut vec![restored],
            ),
            Vec::<RouteMessage>::new()
        );
    }

    #[test]
    fn changed_default_at_another_metric_wins() {
        let original = route([0; 4], 0, 2, 0);
        let mut replacement = route([0; 4], 0, 7, 222);
        replacement.header.kind = 6;
        replacement.header.protocol = 4;
        replacement.nlas.push(Nla::Gateway(vec![192, 0, 2, 8]));
        assert_eq!(
            restoration_plan(
                from_ref(&original),
                &[replacement],
                from_ref(&original),
                &mut Vec::new(),
            ),
            Vec::<RouteMessage>::new()
        );
        assert_eq!(
            restoration_plan(
                from_ref(&original),
                &[],
                from_ref(&original),
                &mut Vec::new(),
            ),
            vec![original]
        );
    }

    #[test]
    fn route_slots_respect_extended_tables_prefixes_and_tos() {
        let original = route([0; 4], 0, 2, 0);
        let mut extended_table = original.clone();
        extended_table.nlas.push(Nla::Table(1000));
        let mut prefix = original.clone();
        prefix.header.destination_prefix_length = 24;
        let mut tos = original.clone();
        tos.header.tos = 4;
        assert_eq!(
            restoration_plan(
                from_ref(&original),
                &[extended_table, prefix, tos],
                from_ref(&original),
                &mut Vec::new(),
            ),
            vec![original.clone()]
        );
        let mut equivalent = original.clone();
        equivalent.header.table = 0;
        equivalent.nlas = vec![Nla::Table(254), Nla::Oif(2)];
        assert!(same_route(&original, &equivalent));
        assert_eq!(
            restoration_plan(
                from_ref(&original),
                &[equivalent],
                from_ref(&original),
                &mut Vec::new(),
            ),
            Vec::<RouteMessage>::new()
        );
    }

    #[test]
    fn mutation_capture_excludes_unchanged_routes_and_other_devices() {
        let original = route([0; 4], 0, 2, 0);
        let unchanged = route([192, 0, 2, 0], 24, 2, 0);
        let unrelated = route([203, 0, 113, 0], 24, 3, 0);
        let mut reordered = unchanged.clone();
        reordered.nlas.reverse();
        reordered.nlas.push(Nla::CacheInfo(vec![1, 2, 3]));
        assert!(route_uses_device(&original, 2));
        assert!(!route_uses_device(&unrelated, 2));
        let mut originals = vec![original.clone(), unchanged];
        retain_displaced_routes(&mut originals, &[reordered, unrelated]);
        assert_eq!(originals, vec![original]);
    }

    #[ignore]
    #[tokio::test]
    async fn test_get_del_routes() {
        let (conn, mut handle, _) = new_connection().unwrap();
        spawn(conn);
        let mut routes = get_routes_noblock(&handle).await.unwrap();
        del_routes_noblock(&handle, routes.clone()).await.unwrap();
        routes.reverse();
        load_routes(&mut handle, routes).await.unwrap();
    }
}
