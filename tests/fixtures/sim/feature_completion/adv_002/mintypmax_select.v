// llg-test-fixture: tests/fixtures/sim/feature_completion/adv_002/mintypmax_select.v
// IEEE 1364-2001 4.3, 7.14.1; IEEE 1800-2009 11.12, 28.16.1. Every
// min:typ:max expression selects its typical value by default, in driver,
// gate, net and procedural delays alike, constant or runtime-valued.
`timescale 1ns/1ns
module tb;
    reg a, b, en;
    reg [7:0] d1, d2, d3;
    wire y_assign, y_gate, y_bufif, y_param, y_sum;
    parameter P = (1:5:9);
    assign #(1:4:9) y_assign = a;
    and #(1:2:9, 2:6:10) g_and (y_gate, a, b);
    bufif1 #(1:2:3, 4:5:6, 7:8:9) g_buf (y_bufif, a, en);
    wire #(1:3:9) y_net;
    assign y_net = a;
    assign #P y_param = a;
    // (a:b:c) + (d:e:f) is b + e (11.12 example 1).
    assign #((1:2:3) + (4:5:6)) y_sum = a;

    initial begin
        $monitor("%0t assign=%b gate=%b bufif=%b net=%b param=%b sum=%b",
                 $time, y_assign, y_gate, y_bufif, y_net, y_param, y_sum);
        d1 = 1; d2 = 3; d3 = 7;
        a = 0; b = 1; en = 1;
        #20 a = 1;
        #20 en = 0;
        #20 ;
        #(d1:d2:d3) $display("%0t runtime", $time);
        #(1:2:3) $display("%0t constant", $time);
        $finish(0);
    end
endmodule
