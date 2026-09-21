// llg-test-fixture: SYN-039 selected pattern/UDP/configuration composition.
// IEEE 1364-2001 §§8.1-8.2 and IEEE 1800-2009 §§7.3.2, 12.6, 29.3-29.4.
// The configured library cell is selected below a design that also exercises
// a combinational UDP and a finite tagged pattern case.
primitive syn039_xor(result, left, right);
    output result;
    input left, right;
    table
        0 0 : 0;
        0 1 : 1;
        1 0 : 1;
        1 1 : 0;
        x ? : x;
        ? x : x;
    endtable
endprimitive

typedef union tagged packed {
    void invalid;
    logic [7:0] valid;
} choice_t;

module pattern_udp (
    input logic [7:0] left,
    input logic [7:0] right,
    output logic [7:0] result
);
    logic parity;
    choice_t choice;

    syn039_xor xor_gate(parity, left[0], right[0]);

    always_comb begin
        choice = tagged valid (left ^ right);
        case (choice) matches
            tagged valid .payload &&& payload == 8'h5a:
                result = payload;
            default:
                result = {7'b0, parity};
        endcase
    end
endmodule

module top;
    logic [7:0] left;
    logic [7:0] right;
    logic [7:0] pattern_result;
    logic [7:0] cell_result;

    pattern_udp pattern(.left(left), .right(right), .result(pattern_result));
    syn039_cell configured(.input_value(left), .output_value(cell_result));

    initial begin
        left = 8'h5a;
        right = 8'h00;
        #1;
        $display("configured=%h pattern=%h", cell_result, pattern_result);
        $finish(0);
    end
endmodule
