// IEEE 1800-2009 §§7.3.1 and 25.5: an equal-width packed union interface
// member is written through one field and passed as a typed child input.
//
// SYN038 selected paths (TY, OP, CO, LV, SL, FM, HC, HR, CP, CT, IN, WK, PC):
//   interface write: untagged_packed_union, direct_projection,
//     assignment_rhs, field, interface_member, none, interface, local, none,
//     none, none, procedural_nba, always_ff.
//   child actual: untagged_packed_union, direct_projection, port_actual, none,
//     interface_member, input, module, interface_member, none, none, none,
//     none, none.
typedef union packed {
    logic [7:0] word;
    logic [1:0][3:0] nibbles;
} union_payload_t;

interface union_if(input logic clk, input logic [7:0] next_word);
    union_payload_t payload;

    always_ff @(posedge clk)
        payload.word <= next_word;
endinterface

module union_observer(
    input union_payload_t value,
    output logic [7:0] word_seen,
    output logic [3:0] high_nibble_seen,
    output logic [3:0] low_nibble_seen
);
    assign word_seen = value.word;
    assign high_nibble_seen = value.nibbles[1];
    assign low_nibble_seen = value.nibbles[0];
endmodule

module tb;
    logic clk;
    logic [7:0] next_word;
    logic [7:0] word_seen;
    logic [3:0] high_nibble_seen;
    logic [3:0] low_nibble_seen;

    union_if bus(.clk(clk), .next_word(next_word));
    union_observer observer(
        .value(bus.payload),
        .word_seen(word_seen),
        .high_nibble_seen(high_nibble_seen),
        .low_nibble_seen(low_nibble_seen)
    );

    initial begin
        clk = 1'b0;
        next_word = 8'h00;
        #1 begin
            next_word = 8'h3a;
            clk = 1'b1;
        end
        #1 begin
            $display("first word=%h nibbles=%h/%h child=%h/%h/%h",
                     bus.payload.word, bus.payload.nibbles[1],
                     bus.payload.nibbles[0], word_seen,
                     high_nibble_seen, low_nibble_seen);
            clk = 1'b0;
            next_word = 8'hc5;
        end
        #1 clk = 1'b1;
        #1 begin
            $display("second word=%h nibbles=%h/%h child=%h/%h/%h",
                     bus.payload.word, bus.payload.nibbles[1],
                     bus.payload.nibbles[0], word_seen,
                     high_nibble_seen, low_nibble_seen);
            $finish(0);
        end
    end
endmodule
