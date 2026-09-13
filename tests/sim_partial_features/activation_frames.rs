use super::run_case;

#[test]
fn loop_captures_are_independent_and_subroutine_calls_reenter() {
    run_case(
        "activation_frames",
        "function formal 5\nfunction capture 105\nfunction formal 10\nfunction capture 110\ntask capture 201\ncapture 0\ncapture 1\ncapture 2\nouter 0\ninner 10\ninner 11\nouter 1\ninner 10\ninner 11\nPASS activation_frames\n",
    );
}

#[test]
fn automatic_real_captures_are_retained_by_detached_forks() {
    run_case(
        "real_activation_capture",
        "real first 1.5\nreal second 2.5\nPASS real_activation_capture\n",
    );
}
