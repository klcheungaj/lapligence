// llg-test-fixture: tests/fixtures/sim/nonconvergence/wait_free_always.sv
// IEEE 1364-2001 §9.9.2: an always procedure repeats even without timing control.
module tb;
    integer x;
    always x = x + 1;
endmodule
