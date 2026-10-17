// llg-test-fixture: tests/fixtures/sim/feature_completion/adv_003/wide_table.v
// IEEE 1364-2001 8.1.2, 8.4, 8.7; IEEE 1800-2009 29.3.2, 29.6, 29.9. A
// ten-input sequential UDP (above the nine-input minimum an implementation
// must accept) with transitions on four inputs: a flip-flop on c/d whose
// level row (all of e1..e8 at 1) dominates the clock edge. A change of e3..e8,
// which no row names with a transition, matches only level rows, so with
// any of them 0 it gives x.
primitive wide_ff (q, c, d, e1, e2, e3, e4, e5, e6, e7, e8);
    output q; reg q;
    input c, d, e1, e2, e3, e4, e5, e6, e7, e8;
    table
    // c d e1 e2 e3 e4 e5 e6 e7 e8 : q : q+
       r 0 ?  ?  ?  ?  ?  ?  ?  ?  : ? : 0 ;
       r 1 ?  ?  ?  ?  ?  ?  ?  ?  : ? : 1 ;
       f ? ?  ?  ?  ?  ?  ?  ?  ?  : ? : - ;
       ? * ?  ?  ?  ?  ?  ?  ?  ?  : ? : - ;
       ? ? *  ?  ?  ?  ?  ?  ?  ?  : ? : - ;
       ? ? ?  *  ?  ?  ?  ?  ?  ?  : ? : - ;
       ? ? 1  1  1  1  1  1  1  1  : ? : 0 ;
    endtable
endprimitive

module tb;
    reg c, d;
    reg [7:0] e;
    wire q;
    wide_ff u (q, c, d, e[0], e[1], e[2], e[3], e[4], e[5], e[6], e[7]);

    task show(input [8*12-1:0] label);
        #1 $display("%0s q=%b", label, q);
    endtask

    initial begin
        c = 1'b0;
        d = 1'b1;
        e = 8'b0000_0000;
        show("start");
        c = 1'b1;      show("rise_1");
        c = 1'b0;      show("fall");
        d = 1'b0;      show("data_0");
        e[0] = 1'b1;   show("e1_edge");
        e[1] = 1'b1;   show("e2_edge");
        d = 1'b1;      show("data_1");
        c = 1'b1;      show("rise_1b");
        e[7] = 1'b1;   show("e8_level_x");
        c = 1'b0;      show("fall_x");
        e = 8'hff;     show("all_ones");
        c = 1'b1;      show("rise_dom");
        $finish(0);
    end
endmodule
