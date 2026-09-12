// llg-test-fixture: tests/fixtures/sim/net_resolution/mixed_biased_structural.sv
module source(input wire i, output wire o);
    assign o = i;
endmodule

module tb;
    reg a, b;
    tri t;
    tri0 t0;
    tri1 t1;
    supply0 s0;
    supply1 s1;

    assign t = a;
    buf gt(t, b);
    source pt(.i(a), .o(t));
    assign t0 = a;
    buf gt0(t0, b);
    source pt0(.i(a), .o(t0));
    assign t1 = a;
    buf gt1(t1, b);
    source pt1(.i(a), .o(t1));
    assign s0 = a;
    buf gs0(s0, b);
    source ps0(.i(a), .o(s0));
    assign s1 = a;
    buf gs1(s1, b);
    source ps1(.i(a), .o(s1));

    initial begin
        a = 0; b = 1; #1;
        $display("first=%b%b%b%b%b", t, t0, t1, s0, s1);
        a = 1; b = 1; #1;
        $display("agree=%b%b%b%b%b", t, t0, t1, s0, s1);
        a = 1'bz; b = 1'bz; #1;
        $display("released=%b%b%b%b%b", t, t0, t1, s0, s1);
        $finish(0);
    end
endmodule
