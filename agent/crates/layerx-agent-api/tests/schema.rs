use std::collections::BTreeMap;

use layerx_agent_api::{
    agent_api_compat_gate, agent_api_schema_v1, Amount, BudgetLimit, ContractVersion, Sequence,
    TimestampSeconds, VersionRequest, VersionResponse, AGENT_API_V1_SOURCE,
};

const BASELINE: &str = include_str!("../../../schema/agent-api/golden/v1.kvx");
const REQUEST_VECTOR: &str = include_str!("../../../schema/agent-api/golden/version-request.hex");
const RESPONSE_VECTOR: &str = include_str!("../../../schema/agent-api/golden/version-response.hex");

fn declarations(source: &str) -> BTreeMap<String, String> {
    let mut section = String::new();
    let mut values = BTreeMap::new();
    for raw in source.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with("//") {
            continue;
        }
        if let Some(name) = line
            .strip_prefix('[')
            .and_then(|value| value.strip_suffix(']'))
        {
            name.clone_into(&mut section);
        } else if let Some((key, value)) = line.split_once('=') {
            values.insert(
                format!("{}.{}", section, key.trim()),
                value.trim().to_owned(),
            );
        }
    }
    values
}

fn hex(value: &str) -> Vec<u8> {
    value
        .trim()
        .as_bytes()
        .chunks_exact(2)
        .map(|digits| {
            let text = std::str::from_utf8(digits)
                .unwrap_or_else(|error| panic!("invalid vector UTF-8: {error}"));
            u8::from_str_radix(text, 16)
                .unwrap_or_else(|error| panic!("invalid vector hex: {error}"))
        })
        .collect()
}

#[test]
fn generated_contract_is_pinned_to_the_schema() {
    let contract = agent_api_schema_v1();
    assert_eq!(contract.name, "LayerX Agent API");
    assert_eq!(contract.version, ContractVersion { major: 1, minor: 1 });
    assert_eq!(contract.node_interface_major, 1);
    let baseline = declarations(BASELINE);
    let current = declarations(AGENT_API_V1_SOURCE);
    for (key, value) in baseline {
        if key == "schema.includes" {
            assert!(
                current[&key].starts_with(value.trim_end_matches(']')),
                "contract module includes were not extended additively"
            );
        } else if key != "schema.minor" {
            assert_eq!(
                current.get(&key),
                Some(&value),
                "contract 1.0 declaration changed at {key}"
            );
        }
    }
    assert_eq!(current.get("schema.minor").map(String::as_str), Some("1"));
    assert!(current["schema.includes"].contains("approval.kvx"));
    assert_eq!(
        current["compatibility.history.1_1.classification"],
        "\"additive_only\""
    );

    let request = VersionRequest {
        request_id: Sequence(7),
        supported: contract.version,
    };
    let response = VersionResponse {
        request_id: request.request_id,
        contract: contract.version,
        node_interface_major: contract.node_interface_major,
    };
    assert_eq!(request.request_id.get(), 7);
    assert_eq!(response.contract, request.supported);
}

#[test]
fn golden_request_and_response_vectors_are_reviewable_exact_bytes() {
    let request = hex(REQUEST_VECTOR);
    let response = hex(RESPONSE_VECTOR);
    assert_eq!(&request[..4], &[0, 1, 0, 0]);
    assert_eq!(&request[4..12], &7_u64.to_be_bytes());
    assert_eq!(&response[..4], &[0, 1, 0, 0]);
    assert_eq!(&response[4..12], &7_u64.to_be_bytes());
    assert!(AGENT_API_V1_SOURCE.contains(REQUEST_VECTOR.trim()));
    assert!(AGENT_API_V1_SOURCE.contains(RESPONSE_VECTOR.trim()));
    assert!(AGENT_API_V1_SOURCE.contains("00010001000000000000000700000000010001"));
    assert!(AGENT_API_V1_SOURCE.contains("0001000100000000000000070000000000000100010001"));
}

#[test]
fn compatibility_gate_accepts_additions_and_rejects_mutation_or_removal() {
    let previous = [("record.Request.fields", "id:u64")];
    let additive = [
        ("record.Request.fields", "id:u64"),
        ("record.Response.fields", "id:u64"),
    ];
    assert_eq!(agent_api_compat_gate(1, 1, &previous, &additive), Ok(()));
    assert!(agent_api_compat_gate(1, 1, &previous, &[]).is_err());
    assert!(
        agent_api_compat_gate(1, 1, &previous, &[("record.Request.fields", "id:u128")]).is_err()
    );
    assert_eq!(agent_api_compat_gate(1, 2, &previous, &[]), Ok(()));
}

#[test]
fn consensus_integers_are_exact_and_dynamic_languages_are_not_numeric() {
    assert_eq!(
        Amount::parse_decimal("340282366920938463463374607431768211455"),
        Ok(Amount(u128::MAX))
    );
    assert!(Amount::parse_decimal("340282366920938463463374607431768211456").is_err());
    assert_eq!(
        BudgetLimit::parse_decimal("9007199254740993"),
        Ok(BudgetLimit(9_007_199_254_740_993))
    );
    assert_eq!(
        Sequence::parse_decimal("18446744073709551615"),
        Ok(Sequence(u64::MAX))
    );
    assert_eq!(
        TimestampSeconds::parse_decimal("42"),
        Ok(TimestampSeconds(42))
    );

    for (key, value) in declarations(AGENT_API_V1_SOURCE) {
        if key.ends_with(".consensus_integer") {
            assert_eq!(value, "true");
        }
        if key.ends_with(".typescript") {
            assert_eq!(value, "\"bigint\"");
        }
        if key.ends_with(".python") {
            assert_eq!(value, "\"int\"");
        }
    }
}
