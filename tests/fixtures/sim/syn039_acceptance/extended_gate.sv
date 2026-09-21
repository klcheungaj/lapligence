// llg-test-fixture: SYN-039 configured library implementation.
module syn039_cell (
    input logic [7:0] input_value,
    output logic [7:0] output_value
);
    assign output_value = input_value + 8'h20;
endmodule

config gate_select;
    design gate.syn039_cell;
endconfig
