// llg-test-fixture: tests/fixtures/sim/feature_completion/adv_003/strengths.v
// IEEE 1364-2001 8.6, 7.9-7.10, 17.1.1.5; IEEE 1800-2009 29.8, 28.11-28.12,
// 21.2.1.5. Sequential UDP outputs drive nets with their declared drive
// strength and resolve with the net's other drivers like gate outputs; an X
// state drives X at that strength. A delayed instance with strengths keeps
// its rise/fall delays.
`timescale 1ns/1ns
primitive latch_udp (q, ena_, data);
    output q; reg q;
    input ena_, data;
    table
    // ena_ data : q : q+
        0    1   : ? : 1 ;
        0    0   : ? : 0 ;
        1    ?   : ? : - ;
        x    0   : 0 : - ;
        x    1   : 1 : - ;
    endtable
endprimitive

module tb;
    reg ena_, d1, d2;
    wire w1, w2, w3, w4;
    latch_udp (pull0, pull1) u1 (w1, ena_, d1);
    latch_udp (strong0, strong1) u2 (w2, ena_, d1);
    latch_udp u3 (w2, ena_, d2);
    latch_udp (weak0, weak1) u4 (w3, ena_, d1);
    assign (pull0, pull1) w3 = d2;
    latch_udp (pull0, pull1) #(2, 3) u5 (w4, ena_, d1);

    task show(input [8*8-1:0] label);
        $display("%0s w1=%v w2=%v w3=%v w4=%v", label, w1, w2, w3, w4);
    endtask

    initial begin
        ena_ = 1'b0;
        d1 = 1'b1;
        d2 = 1'b1;
        #1 show("t1");
        #2 show("t3");
        d2 = 1'b0;
        #1 show("d2_0");
        d1 = 1'b0;
        #2 show("t6");
        #1 show("t7");
        d1 = 1'b1;
        #4 show("d1_1");
        ena_ = 1'bx;
        #4 show("ena_x");
        d1 = 1'b0;
        #1 show("t16");
        #2 show("t18");
        $finish(0);
    end
endmodule
