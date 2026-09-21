// llg-test-fixture: tests/fixtures/sim/rtl_completion/syn_006_array_continuous_variable_conflict.sv
// LRM: IEEE 1800-2009 §§6.8, 7.6, 10.3; IEEE 1800-2005 §6.5.
module tb;
    logic [7:0] a [0:1];
    logic [7:0] b [0:1];
    logic [7:0] y [0:1];

    assign y = a;
    assign y = b;
endmodule
