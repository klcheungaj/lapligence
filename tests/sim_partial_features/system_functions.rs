use super::run_case;

#[test]
fn real_math_functions_accept_runtime_values_and_numeric_conversions() {
    run_case(
        "math_runtime",
        "4.000000 256.000000 2.000000 2.000000\n\
         -3.000000 -2.000000 9.000000 4294967296.000000\n\
         0.500000 0.500000 0.250000\n\
         3.141593 5.000000\n\
         0.500000 2.000000 0.250000\n\
         direct 2.772589 1.204120 7.389056 4.000000 256.000000\n\
         trig 0.479426 0.877583 0.546302 0.523599 1.047198 0.463648\n\
         pair -0.463648 16.124515\n\
         hyper 0.521095 1.127626 0.462117 0.481212 3.464758 0.549306\n\
         round -3.000000 -2.000000\n",
    );
}

#[test]
fn real_math_evaluates_arguments_once_and_preserves_c_domain_behavior() {
    run_case("math_evaluation", "calls 1 1.0\ndomain 1 1\n");
}

#[test]
fn realtime_preserves_fractional_time_in_the_calling_module_unit() {
    run_case(
        "realtime_units",
        "parent 0.125\nchild 0.025\nparent 0.375\n",
    );
}
