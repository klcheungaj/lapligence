use super::run_case;

#[test]
fn timeformat_defaults_dynamic_state_and_output_families() {
    run_case(
        "timeformat_q05",
        "default=[                1000]\n\
dynamic=[0.00 us]\n\
changed=[ 1.00 ns] now=[ 1.00 ns] real=[ 1.23 ns]\n\
integer=[1234.00 ns]\n\
signed=[-1234.00 ns]\n\
rounded=[ 1.00 ns  2.00 ns  3.00 ns]\n\
half=[0.12 us 0.13 us 0.14 us -0.12 us -0.13 us -0.14 us]\n\
realhalf=[0.12 us -0.12 us]\n\
wide=[123456789012345678901234.00 ns]\n\
explicit=[   1.00 ns]\n\
small=[1.00 ns]\n\
zero=[1.00 ns]\n\
high=[1.00000000000000 ns]\n\
ps=[1000.000 ps]\n\
write=[1.2 ns]\n\
reset=[                1000]\n\
strobe=[                1000]\n\
monitor=[                1000] changed=1\n\
final=[                2000]\n",
    );
}

#[test]
fn timeformat_converts_mixed_scope_time_values() {
    run_case(
        "timeformat_mixed_scopes",
        "parent=[1.000 ns] parentreal=[1.234 ns]\n\
child=[1000.000 ns] childreal=[1234.000 ns]\n",
    );
}
