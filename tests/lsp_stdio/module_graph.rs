//! Module graph.

use super::*;

/// The module explorer is an end-to-end read of the committed analysis.  The
/// configured top deliberately leaves `unrelated` out of the elaborated
/// hierarchy while the source graph still sees every definition and source edge. This catches
/// both the top-child-leaf root regression and the declaration-only contents
/// fallback over the actual JSON-RPC boundary.
#[test]
fn lsp_stdio_module_explorer_uses_source_graph_and_declaration_fallback() {
    let fixture = FixtureTree::module_explorer();
    let ws = fixture.root("workspace");

    let mut client = LspProcess::spawn(&ws);
    client
        .initialize(&[("module-explorer", &ws)], default_init_options())
        .expect("initialize module-explorer workspace");
    let snapshot = client
        .request("llg/moduleExplorer", json!({}))
        .expect("module explorer request");
    assert_no_shadow_uris(&snapshot);

    let roots = snapshot
        .get("roots")
        .and_then(Value::as_array)
        .expect("module explorer roots array");
    let root_names = roots
        .iter()
        .filter_map(|root| root.get("moduleType").and_then(Value::as_str))
        .collect::<Vec<_>>();
    assert!(
        root_names.contains(&"top"),
        "configured top must be a root: {snapshot}"
    );
    assert!(
        root_names.contains(&"unrelated"),
        "uninstantiated source definition must remain a root: {snapshot}"
    );
    assert!(
        !root_names.contains(&"child"),
        "instantiated child must not be a root: {snapshot}"
    );
    assert!(
        !root_names.contains(&"leaf"),
        "transitive leaf must not be a root: {snapshot}"
    );

    let top_root = roots
        .iter()
        .find(|root| root.get("moduleType").and_then(Value::as_str) == Some("top"))
        .expect("top root occurrence");
    let child = top_root
        .get("children")
        .and_then(Value::as_array)
        .and_then(|children| {
            children
                .iter()
                .find(|child| child.get("instanceName").and_then(Value::as_str) == Some("u_child"))
        })
        .expect("top.u_child nested occurrence");
    assert_eq!(
        child.get("moduleType").and_then(Value::as_str),
        Some("child")
    );
    assert_eq!(
        child.get("contentSource").and_then(Value::as_str),
        Some("elaborated")
    );
    assert_eq!(
        child
            .get("children")
            .and_then(Value::as_array)
            .and_then(|children| children.first())
            .and_then(|leaf| leaf.get("moduleType"))
            .and_then(Value::as_str),
        Some("leaf")
    );

    let child_port = child
        .get("ports")
        .and_then(Value::as_array)
        .and_then(|ports| {
            ports
                .iter()
                .find(|port| port.get("name").and_then(Value::as_str) == Some("clk"))
        })
        .expect("elaborated child port");
    assert!(
        child_port
            .get("location")
            .and_then(|location| location.get("uri"))
            .and_then(Value::as_str)
            .is_some_and(|uri| uri.ends_with("/workspace/child.sv")),
        "child port location must point at its declaration: {child_port}"
    );
    assert_eq!(
        child_port
            .get("location")
            .and_then(|location| location.get("range")),
        Some(&json!({
            "startLine": 1,
            "startCharacter": 52,
            "endLine": 1,
            "endCharacter": 55
        })),
        "child port location must cover the clk identifier: {child_port}"
    );
    let child_param = child
        .get("params")
        .and_then(Value::as_array)
        .and_then(|params| {
            params
                .iter()
                .find(|param| param.get("name").and_then(Value::as_str) == Some("WIDTH"))
        })
        .expect("elaborated child parameter");
    assert_eq!(
        child_param.get("value").and_then(Value::as_str),
        Some("32'sd8")
    );
    assert!(
        child_param
            .get("location")
            .and_then(|location| location.get("uri"))
            .and_then(Value::as_str)
            .is_some_and(|uri| uri.ends_with("/workspace/child.sv")),
        "child parameter location must point at its declaration: {child_param}"
    );
    assert_eq!(
        child_param
            .get("location")
            .and_then(|location| location.get("range")),
        Some(&json!({
            "startLine": 1,
            "startCharacter": 29,
            "endLine": 1,
            "endCharacter": 34
        })),
        "child parameter location must cover the WIDTH identifier: {child_param}"
    );
    let child_signal = child
        .get("signals")
        .and_then(Value::as_array)
        .and_then(|signals| {
            signals
                .iter()
                .find(|signal| signal.get("name").and_then(Value::as_str) == Some("payload"))
        })
        .expect("elaborated child signal");
    assert_eq!(
        child_signal
            .get("type")
            .and_then(|ty| ty.get("displayType"))
            .and_then(Value::as_str),
        Some("logic [7:0]"),
        "child signal width must use the exact elaborated WIDTH value: {child_signal}"
    );
    assert_eq!(
        child_signal
            .get("type")
            .and_then(|ty| ty.get("width"))
            .and_then(Value::as_u64),
        Some(8)
    );
    assert!(
        child_signal
            .get("location")
            .and_then(|location| location.get("uri"))
            .and_then(Value::as_str)
            .is_some_and(|uri| uri.ends_with("/workspace/child.sv")),
        "child signal location must point at its declaration: {child_signal}"
    );
    assert_eq!(
        child_signal
            .get("location")
            .and_then(|location| location.get("range")),
        Some(&json!({
            "startLine": 4,
            "startCharacter": 4,
            "endLine": 4,
            "endCharacter": 11
        })),
        "child signal location must cover the payload identifier: {child_signal}"
    );
    let child_memory = child
        .get("signals")
        .and_then(Value::as_array)
        .and_then(|signals| {
            signals
                .iter()
                .find(|signal| signal.get("name").and_then(Value::as_str) == Some("memory"))
        })
        .expect("elaborated packed-plus-unpacked child signal");
    assert_eq!(
        child_memory
            .get("type")
            .and_then(|ty| ty.get("displayType"))
            .and_then(Value::as_str),
        Some("logic [7:0] [0:1]"),
        "packed width must come from elaboration while the unpacked range stays source-backed: {child_memory}"
    );
    assert_eq!(
        child_memory
            .get("type")
            .and_then(|ty| ty.get("width"))
            .and_then(Value::as_u64),
        Some(8)
    );
    assert_eq!(
        child_memory.get("kind").and_then(Value::as_str),
        Some("array"),
        "the concrete elaborated array kind must remain intact: {child_memory}"
    );
    let leaf = child
        .get("children")
        .and_then(Value::as_array)
        .and_then(|children| children.first())
        .expect("nested leaf occurrence");
    let leaf_signal = leaf
        .get("signals")
        .and_then(Value::as_array)
        .and_then(|signals| signals.first())
        .expect("nested leaf signal");
    assert_eq!(
        leaf_signal
            .get("type")
            .and_then(|ty| ty.get("displayType"))
            .and_then(Value::as_str),
        Some("logic [5:0]"),
        "nested signal width must use the nested instance parameter: {leaf_signal}"
    );
    assert!(
        leaf_signal
            .get("location")
            .and_then(|location| location.get("uri"))
            .and_then(Value::as_str)
            .is_some_and(|uri| uri.ends_with("/workspace/leaf.sv")),
        "nested signal location must point at its declaration: {leaf_signal}"
    );

    let top_signal = top_root
        .get("signals")
        .and_then(Value::as_array)
        .and_then(|signals| signals.first())
        .expect("top internal signal");
    assert!(
        top_signal
            .get("location")
            .and_then(|location| location.get("uri"))
            .and_then(Value::as_str)
            .is_some_and(|uri| uri.ends_with("/workspace/top.sv")),
        "top signal location must point at its declaration: {top_signal}"
    );

    let unrelated = roots
        .iter()
        .find(|root| root.get("moduleType").and_then(Value::as_str) == Some("unrelated"))
        .expect("unrelated declaration-only root");
    assert_eq!(
        unrelated.get("contentSource").and_then(Value::as_str),
        Some("declaration")
    );
    let unrelated_port = unrelated
        .get("ports")
        .and_then(Value::as_array)
        .and_then(|ports| ports.first())
        .expect("unrelated formal port");
    assert_eq!(
        unrelated_port.get("detail").and_then(Value::as_str),
        Some("input logic pin"),
        "inline declaration details must be scoped to the selected identifier: {unrelated}"
    );
    assert_eq!(
        unrelated
            .get("signals")
            .and_then(Value::as_array)
            .map(|signals| signals
                .iter()
                .filter_map(|signal| signal.get("name").and_then(Value::as_str))
                .collect::<Vec<_>>()),
        Some(vec!["internal_bus"])
    );
    let unrelated_signal = unrelated
        .get("signals")
        .and_then(Value::as_array)
        .and_then(|signals| signals.first())
        .expect("unrelated internal signal");
    assert_eq!(
        unrelated_signal
            .get("type")
            .and_then(|ty| ty.get("kind"))
            .and_then(Value::as_str),
        Some("logic"),
        "declaration fallback must retain the source element type: {unrelated}"
    );
    assert_eq!(
        unrelated_signal
            .get("type")
            .and_then(|ty| ty.get("width"))
            .and_then(Value::as_u64),
        Some(8),
        "declaration fallback must retain literal packed width: {unrelated}"
    );
    assert_eq!(
        unrelated_signal
            .get("type")
            .and_then(|ty| ty.get("displayType"))
            .and_then(Value::as_str),
        Some("logic [7:0]"),
        "declaration fallback must retain a sanitized display type: {unrelated}"
    );
    assert!(
        unrelated_signal
            .get("location")
            .and_then(|location| location.get("uri"))
            .and_then(Value::as_str)
            .is_some_and(|uri| uri.ends_with("/workspace/unrelated.sv")),
        "declaration fallback signal location must point at its declaration: {unrelated}"
    );
    assert!(
        unrelated_port
            .get("location")
            .and_then(|location| location.get("uri"))
            .and_then(Value::as_str)
            .is_some_and(|uri| uri.ends_with("/workspace/unrelated.sv")),
        "declaration fallback port location must point at its declaration: {unrelated}"
    );
    assert!(
        unrelated
            .get("signals")
            .and_then(Value::as_array)
            .is_some_and(|signals| {
                signals.iter().all(|signal| {
                    !matches!(
                        signal.get("name").and_then(Value::as_str),
                        Some("function_local") | Some("task_local")
                    )
                })
            }),
        "function/task locals must not leak into module signals: {unrelated}"
    );
    assert!(
        unrelated
            .get("signals")
            .and_then(Value::as_array)
            .is_some_and(|signals| {
                signals
                    .iter()
                    .all(|signal| signal.get("name").and_then(Value::as_str) != Some("pin"))
            }),
        "formal ports must not be repeated as signals: {unrelated}"
    );
    assert!(
        snapshot
            .get("modules")
            .and_then(Value::as_array)
            .is_some_and(|modules| modules.iter().any(|module| {
                module.get("name").and_then(Value::as_str) == Some("unrelated")
                    && module.get("contentSource").and_then(Value::as_str) == Some("declaration")
            })),
        "module definition fallback must be present: {snapshot}"
    );

    client.shutdown();
}
