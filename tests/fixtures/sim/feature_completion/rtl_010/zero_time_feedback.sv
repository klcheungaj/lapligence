// IEEE 1364-2005 6.1.2 and IEEE 1800-2009 10.3.2: a continuous assignment is
// re-evaluated whenever an operand changes, including a change its own
// publication causes. These loops converge in zero time.
module tb;
    typedef logic [1:0][3:0] pair_t;
    bit [3:0] count;
    bit go = 0;
    wire [3:0] a, b;
    wire [3:0] p, q;
    alias p = q;
    logic [3:0] arr[0:2];
    wire [3:0] n[0:2];
    logic [3:0] seed = 4'h1;
    // Counts up to five, one re-evaluation per step.
    assign count = go ? (count < 4'd5 ? count + 4'd1 : count) : 4'd0;
    // Both leaves read the other; an X on one settles both.
    assign '{a, b} = pair_t'({b + 4'(go), a});
    // A write through one alias view changes the operand read through the other.
    assign p = go ? (q < 4'd6 ? q + 4'd1 : q) : 4'd0;
    // Each cell reads its predecessor, so the array settles in three passes.
    assign arr = '{seed, arr[0] + 4'd1, arr[1] + 4'd1};
    assign n = '{seed, n[0] + 4'd2, n[1] + 4'd2};
    initial begin
        #1 $display("%h %h %h %h | %h %h %h | %h %h %h", count, a, b, q, arr[0], arr[1], arr[2],
                    n[0], n[1], n[2]);
        go = 1;
        seed = 4'h7;
        #1 $display("%h %h %h %h | %h %h %h | %h %h %h", count, a, b, q, arr[0], arr[1], arr[2],
                    n[0], n[1], n[2]);
        $finish(0);
    end
endmodule
