use super::run_case;

#[test]
fn loop_variables_are_shared_and_subroutine_calls_reenter() {
    run_case(
        "activation_frames",
        "function formal 5\nfunction capture 105\nfunction formal 10\nfunction capture 110\ntask capture 201\ncapture 3\ncapture 3\ncapture 3\nouter 2\ninner 12\ninner 12\nouter 2\ninner 12\ninner 12\nPASS activation_frames\n",
    );
}

#[test]
fn automatic_real_captures_are_retained_by_detached_forks() {
    run_case(
        "real_activation_capture",
        "real first 9.5\nreal second 8.5\nPASS real_activation_capture\n",
    );
}
