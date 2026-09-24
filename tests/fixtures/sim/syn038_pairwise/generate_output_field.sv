// llg-test-fixture: tests/fixtures/sim/syn038_pairwise/generate_output_field.sv
// IEEE 1800-2009 §§7.2, 23.3.3, and 27.4: a generated child output drives
// one packed-struct field actual while the parent writes its sibling field.
typedef struct packed {
    logic [7:0] hi;
    logic [7:0] lo;
} pair_t;

module field_source(input logic [7:0] source, output logic [7:0] y);
    assign y = source;
endmodule

module tb;
    pair_t pair;
    logic [7:0] source;

    if (1'b1) begin : generated
        field_source u_field(.source(source), .y(pair.hi));
    end

    initial begin
        source = 8'h5a;
        pair.lo = 8'hc3;
        #1;
        if (pair.hi !== 8'h5a || pair.lo !== 8'hc3)
            $fatal(1, "generated child output did not reach the packed field");
        $display("hi=%h", pair.hi);
        $finish(0);
    end
endmodule
