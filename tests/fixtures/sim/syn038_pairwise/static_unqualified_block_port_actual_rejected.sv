// llg-test-fixture: tests/fixtures/sim/syn038_pairwise/static_unqualified_block_port_actual_rejected.sv
`default_nettype none
module leaf(input logic [7:0] value);
endmodule

module tb;
    leaf u(.value(static_value));

    initial begin : named_process
        static logic [7:0] static_value;
        static_value = 8'h31;
    end
endmodule
`default_nettype wire
