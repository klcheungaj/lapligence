// llg-test-fixture: tests/fixtures/sim/feature_completion/adv_003/latch.v
// IEEE 1364-2001 8.1.4-8.1.6, 8.3; IEEE 1800-2009 29.3.4-29.3.6, 29.5.
// Level-sensitive sequential UDP: level symbols 0 1 x ? b in input and
// current-state fields, `-` (no change), an input Z read as X, and an
// unmatched combination giving X. Each step changes one input.
primitive latch_udp (q, ena_, data);
    output q;
    reg q;
    input ena_, data;
    table
    // ena_ data : q : q+
        0    1   : ? : 1 ;
        0    0   : ? : 0 ;
        1    ?   : ? : - ;
        x    0   : 0 : - ;
        x    1   : 1 : - ;
        b    x   : x : x ;
    endtable
endprimitive

module tb;
    reg ena_, data;
    wire q;
    latch_udp u (q, ena_, data);

    task show(input [8*12-1:0] label);
        #1 $display("%0s q=%b", label, q);
    endtask

    initial begin
        show("start");
        ena_ = 1'b0;
        show("ena0");
        data = 1'b1;
        show("load1");
        data = 1'b0;
        show("load0");
        ena_ = 1'b1;
        show("close");
        data = 1'b1;
        show("hold_d1");
        data = 1'bz;
        show("hold_dz");
        ena_ = 1'bx;
        show("ena_x");
        data = 1'b0;
        show("ena_x_d0");
        ena_ = 1'b0;
        show("load0b");
        ena_ = 1'bx;
        show("x_same");
        data = 1'b1;
        show("x_diff");
        ena_ = 1'b0;
        show("load1b");
        ena_ = 1'bz;
        show("z_same");
        ena_ = 1'b1;
        show("z_to_1");
        $finish(0);
    end
endmodule
