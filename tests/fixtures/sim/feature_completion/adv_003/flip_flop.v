// llg-test-fixture: tests/fixtures/sim/feature_completion/adv_003/flip_flop.v
// IEEE 1364-2001 8.4; IEEE 1800-2009 29.6. The d_edge_ff example with
// explicit transitions (01), (0?), (?0) and (??), its stated case (clock
// 0 -> x with data 0 and state 1 gives x), and unspecified transitions
// (x -> 1, 1 -> z) giving X. Each step changes one input.
primitive d_edge_ff (q, clock, data);
    output q; reg q;
    input clock, data;
    table
    // clock data : q : q+
        (01)  0   : ? : 0 ;
        (01)  1   : ? : 1 ;
        (0?)  1   : 1 : 1 ;
        (0?)  0   : 0 : 0 ;
        (?0)  ?   : ? : - ;
         ?   (??) : ? : - ;
    endtable
endprimitive

module tb;
    reg clock, data;
    wire q;
    d_edge_ff u (q, clock, data);

    task show(input [8*12-1:0] label);
        #1 $display("%0s clock=%b data=%b q=%b", label, clock, data, q);
    endtask

    initial begin
        show("start");
        clock = 1'b0;  show("x_to_0");
        data = 1'b1;   show("data_x_1");
        clock = 1'b1;  show("rise_1");
        clock = 1'b0;  show("fall");
        data = 1'b0;   show("data_0");
        clock = 1'bx;  show("0x_d0_s1");
        clock = 1'b0;  show("x0");
        clock = 1'b1;  show("rise_0");
        clock = 1'b0;  show("fall_0");
        clock = 1'bx;  show("0x_d0_s0");
        clock = 1'b1;  show("x1");
        data = 1'b1;   show("data_1");
        clock = 1'b0;  show("fall_x");
        clock = 1'b1;  show("rise_1b");
        data = 1'bz;   show("data_z");
        clock = 1'bz;  show("1z");
        $finish(0);
    end
endmodule
