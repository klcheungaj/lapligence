// IEEE 1800-2009 7.2.1 and 7.4.1: packed structures and multidimensional
// packed arrays have a contiguous integral layout with left dimensions first.
module tb #(parameter WIDTH = 2048, parameter HALF = WIDTH / 2);
    typedef struct packed {
        logic [HALF-1:0] upper;
        logic [HALF-1:0] lower;
    } pair_t;
    typedef struct packed signed {
        logic [WIDTH-2:0] head;
        logic tail;
    } signed_pair_t;

    pair_t pair;
    signed_pair_t signed_pair;
    logic [1:0][HALF-1:0] lanes;
    logic [WIDTH-1:0] flat;
    logic [WIDTH-1:0] expected;
    logic signed [WIDTH:0] extended;
    integer failed;

    initial begin
        failed = 0;
        pair = '0;
        pair.upper[HALF-1] = 1'b1;
        pair.upper[0] = 1'b1;
        pair.lower[64] = 1'b1;
        pair.lower[0] = 1'b1;
        #1;

        flat = pair;
        expected = '0;
        expected[WIDTH-1] = 1'b1;
        expected[HALF] = 1'b1;
        expected[64] = 1'b1;
        expected[0] = 1'b1;
        if (flat !== expected) begin
            $display("FAIL struct-layout WIDTH=%0d", WIDTH);
            failed = 1;
        end

        lanes = flat;
        if (!failed &&
            (lanes[1] !== pair.upper || lanes[0] !== pair.lower)) begin
            $display("FAIL packed-array-layout WIDTH=%0d", WIDTH);
            failed = 1;
        end

        lanes[1][0] = 1'b0;
        pair = lanes;
        if (!failed &&
            (pair.upper[0] !== 1'b0 || pair.lower[0] !== 1'b1 ||
             pair.upper[HALF-1] !== 1'b1 || pair.lower[64] !== 1'b1)) begin
            $display("FAIL member-select-update WIDTH=%0d", WIDTH);
            failed = 1;
        end

        signed_pair = '0;
        signed_pair.head[WIDTH-2] = 1'b1;
        extended = signed_pair;
        if (!failed && extended[WIDTH:WIDTH-1] !== 2'b11) begin
            $display("FAIL signed-struct-extension WIDTH=%0d", WIDTH);
            failed = 1;
        end

        if (!failed) $display("PASS packed_aggregates WIDTH=%0d", WIDTH);
        $finish;
    end
endmodule
