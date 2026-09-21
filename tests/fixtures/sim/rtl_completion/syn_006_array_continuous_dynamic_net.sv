// llg-test-fixture: tests/fixtures/sim/rtl_completion/syn_006_array_continuous_dynamic_net.sv
// LRM: IEEE 1800-2009 §§7.4.2, 7.4.6, 10.3.
module tb;
    logic [7:0] source [0:1];
    wire [7:0] target [0:1];
    logic index;

    assign target[index] = source[index];
endmodule
