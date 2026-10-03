// llg-test-fixture: electrical runs retain four-state values and declared shapes.
module collapsed #(parameter W = 7)(inout wire [W-1:0] bus);
    assign bus = 'z;
endmodule
module wired_collapse #(parameter W = 7)(inout wand [W-1:0] bus);
    assign bus = '0;
    assign bus = '1;
endmodule
module partition_case #(parameter W = 7)(output bit done);
    logic [W-1:0] a = '0, b = '1;
    wire [W-1:0] single[2], full[2], overlap[2], disjoint[2], ported[2];
    wire [W-1:0] aliased[2], delayed[2];
    wand [W-1:0] anded[2];
    wor [W-1:0] ored[2];
    wire [W-1:0] peer;
    wire [W-1:0] strength_bits;
    localparam H = W / 2;
    localparam L = W / 3;
    assign single[0] = a;
    assign full[0] = a;
    assign full[0] = b;
    assign overlap[0][W-1:L] = a[W-1:L];
    assign overlap[0][H:0] = b[H:0];
    assign disjoint[0][W-1:H] = a[W-1:H];
    if (H > 0) begin : low_disjoint
        assign disjoint[0][H-1:0] = b[H-1:0];
    end
    assign ported[0] = a;
    collapsed #(W) link(ported[0]);
    wired_collapse #(W) typed(ported[1]);
    alias peer = aliased[0];
    assign peer = a;
    assign aliased[0] = b;
    assign anded[0] = a;
    assign anded[0] = b;
    assign ored[0] = a;
    assign ored[0] = b;
    assign #2 delayed[0] = a;
    assign #2 delayed[0] = b;
    assign #1 strength_bits = a;
    for (genvar i = 0; i < W; i++) begin : strengths
        wire driven;
        assign (strong0, strong1) driven = 1'b0;
        assign (weak0, weak1) driven = 1'b1;
        alias driven = strength_bits[i];
    end
    initial begin
        done = 0;
        #3;
        $display("%m %b %b %b %b %b %b %b %b %b %b %b", single[0], full[0],
            overlap[0], disjoint[0], ported[0], ported[1], aliased[0],
            anded[0], ored[0], strength_bits, delayed[0]);
        force peer[H] = 1'b1;
        #1;
        $display("%m bit %b", aliased[0]);
        release peer[H];
        force peer[W-1:H] = '0;
        a = '1;
        b = 'z;
        #3;
        $display("%m range %b %b", aliased[0], delayed[0]);
        release peer[W-1:H];
        #1;
        $display("%m release %b %b", peer, aliased[0]);
        done = 1;
    end
endmodule
module tb;
    wire [4:0] done;
    partition_case #(1) w1(done[0]);
    partition_case #(7) w7(done[1]);
    partition_case #(64) w64(done[2]);
    partition_case #(65) w65(done[3]);
    partition_case #(129) w129(done[4]);
    initial begin
        #10;
        if (done !== '1) $fatal(1, "partition cases incomplete");
        $finish(0);
    end
endmodule
