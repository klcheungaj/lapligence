//! KI continuous dynamic reads: a reference to an element or member of a
//! dynamic variable (a queue, dynamic or associative array, string, class
//! object or virtual interface) in a continuous context is a located
//! frontend error naming the clauses (IEEE 1800-2009 6.21, 25.9). Every
//! continuous form (an assignment, a net declaration assignment, an input
//! port connection, a gate terminal) and every kind of reference rejects at
//! the reference; the fixture readme quotes the clauses.
use super::sim_cli;

const SUITE: &str = "feature_completion/ki_continuous_dynamic_reads";

const ILLEGAL: &str = "cannot refer to an element or member of a dynamic type outside of a \
                       procedural context (IEEE 1800-2009 6.21; 25.9 for a virtual interface)";

const ID: &str = "SLANG-DynamicNotProcedural";

#[test]
fn continuous_contexts_reject_dynamic_elements() {
    sim_cli::reject_case_with_id(SUITE, "neg_assign_queue_element", ID, Some("6:14"), ILLEGAL);
    sim_cli::reject_case_with_id(
        SUITE,
        "neg_assign_dynamic_element",
        ID,
        Some("6:14"),
        ILLEGAL,
    );
    sim_cli::reject_case_with_id(
        SUITE,
        "neg_net_declaration_associative_element",
        ID,
        Some("5:19"),
        ILLEGAL,
    );
    sim_cli::reject_case_with_id(
        SUITE,
        "neg_input_port_queue_element",
        ID,
        Some("9:15"),
        ILLEGAL,
    );
    sim_cli::reject_case_with_id(
        SUITE,
        "neg_input_port_associative_element",
        ID,
        Some("8:15"),
        ILLEGAL,
    );
    sim_cli::reject_case_with_id(SUITE, "neg_gate_queue_element", ID, Some("6:13"), ILLEGAL);
}

#[test]
fn continuous_contexts_reject_members_and_nested_elements() {
    sim_cli::reject_case_with_id(
        SUITE,
        "neg_assign_class_property",
        ID,
        Some("9:14"),
        ILLEGAL,
    );
    sim_cli::reject_case_with_id(
        SUITE,
        "neg_assign_virtual_interface_member",
        ID,
        Some("10:14"),
        ILLEGAL,
    );
    sim_cli::reject_case_with_id(
        SUITE,
        "neg_assign_record_element_member",
        ID,
        Some("9:14"),
        ILLEGAL,
    );
    sim_cli::reject_case_with_id(
        SUITE,
        "neg_assign_nested_queue_element",
        ID,
        Some("6:14"),
        ILLEGAL,
    );
    sim_cli::reject_case_with_id(SUITE, "neg_assign_call_actual", ID, Some("9:16"), ILLEGAL);
    sim_cli::reject_case_with_id(SUITE, "neg_assign_string_byte", ID, Some("6:14"), ILLEGAL);
    sim_cli::reject_case_with_id(
        SUITE,
        "neg_process_status_continuous",
        ID,
        Some("5:31"),
        ILLEGAL,
    );
    sim_cli::reject_case_with_id(SUITE, "neg_randomize_continuous", ID, Some("9:14"), ILLEGAL);
}
