// llg-test-fixture: IEEE 1800-2009 §§6.22, 7.3.1, 13.4, 23.2.2.2, and 9.2.2.2.
// A packed four-state union function result feeds a child input; a matching
// child output writes a separate union actual. A separate clocked module
// captures one named union field through a conditional nonblocking assignment.
typedef struct packed {
    logic [7:0] high;
    logic [7:0] low;
} halves_t;

typedef union packed {
    logic [15:0] word;
    halves_t halves;
    logic signed [15:0] signed_word;
} payload_t;

module union_echo(input payload_t payload, output payload_t echoed);
    assign echoed = payload;
endmodule

module union_field_ff(
    input logic clk,
    input logic choose_true,
    input payload_t true_value,
    input payload_t false_value,
    output logic [7:0] captured
);
    payload_t state;

    always_ff @(posedge clk)
        state.halves.low <= choose_true ? true_value.halves.low : false_value.halves.low;

    always_comb captured = state.halves.low;
endmodule

module tb;
    logic clk;
    logic choose_true;
    payload_t true_value;
    payload_t false_value;
    payload_t echoed_actual;
    logic [7:0] captured;

    function automatic payload_t make_payload(input logic [15:0] bits);
        payload_t result;
        result.word = bits;
        return result;
    endfunction

    union_echo u_echo(
        .payload(make_payload(16'hA5z3)),
        .echoed(echoed_actual)
    );

    union_field_ff u_field_ff(
        .clk(clk),
        .choose_true(choose_true),
        .true_value(true_value),
        .false_value(false_value),
        .captured(captured)
    );

    initial begin
        clk = 1'b0;
        choose_true = 1'bx;
        true_value.word = 16'h12a5;
        false_value.word = 16'h345a;

        #1;
        clk = 1'b1;
        #1;
        if (echoed_actual.word !== 16'hA5z3 ||
            echoed_actual.halves.high !== 8'hA5 ||
            echoed_actual.halves.low !== 8'hz3 ||
            captured !== 8'hxx)
            $fatal(1, "packed union function/port/field paths produced wrong values");
        $display("echo=%h halves=%h,%h captured=%h", echoed_actual.word,
                 echoed_actual.halves.high, echoed_actual.halves.low, captured);
        $finish(0);
    end
endmodule
