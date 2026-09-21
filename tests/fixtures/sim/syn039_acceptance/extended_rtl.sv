// llg-test-fixture: SYN-039 unselected library alternative.
module syn039_cell (
    input logic [7:0] input_value,
    output logic [7:0] output_value
);
    assign output_value = input_value + 8'h10;
endmodule
