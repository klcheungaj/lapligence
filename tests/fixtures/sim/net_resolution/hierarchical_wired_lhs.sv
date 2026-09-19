// A hierarchical continuous-assignment LHS that the owned database resolves to
// a wired net is a real driver identity, both for a top-self path and for a
// selected net inside a child instance. A concatenation that contains a wired
// net contributes only its own bits; undriven bits stay high-Z. LRM: IEEE
// 1800-2009 6.5, 10.3.
module hier_child;
    wand [1:0] w;
endmodule

module tb;
    logic a, b;
    wand self_w;
    hier_child u();

    wand [3:0] concat_w;
    logic [1:0] concat_v;
    logic concat_x;

    assign tb.self_w = a;
    assign u.w[0] = a;
    assign u.w[1] = b;
    assign {concat_w[3:2], concat_v, concat_x} = 5'b10101;

    initial begin
        a = 1'b1; b = 1'b0; #1;
        $display("CHECK: self=%b child=%b concat=%b cv=%b cx=%b",
                 self_w, u.w, concat_w, concat_v, concat_x);
        a = 1'b0; b = 1'b1; #1;
        $display("CHECK: self=%b child=%b concat=%b cv=%b cx=%b",
                 self_w, u.w, concat_w, concat_v, concat_x);
        a = 1'bz; b = 1'b1; #1;
        $display("CHECK: self=%b child=%b concat=%b cv=%b cx=%b",
                 self_w, u.w, concat_w, concat_v, concat_x);
        $finish(0);
    end
endmodule
