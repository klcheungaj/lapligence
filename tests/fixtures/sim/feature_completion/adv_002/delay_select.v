// llg-test-fixture: tests/fixtures/sim/feature_completion/adv_002/delay_select.v
// IEEE 1364-2001 4.3; IEEE 1800-2009 11.12, 28.16.1. One min:typ:max delay of
// each form (continuous assignment, gate, UDP, net, parameter, specify path
// with a specparam, procedural constant and run-time delays, and a parameter
// expression) run under each `--delay-select` value; the expected output of
// each selection is delay_select_<min|typ|max>.out.
`timescale 1ns/1ns
primitive u_buf (out, in);
    output out;
    input in;
    table
        0 : 0;
        1 : 1;
    endtable
endprimitive

module path_buf (y, a);
    output y;
    input a;
    specify
        specparam TPD = (6:16:26);
        (a => y) = TPD;
    endspecify
    buf (y, a);
endmodule

module tb;
    reg a;
    reg [7:0] d1, d2, d3;
    wire w_assign, w_gate, w_udp, w_param, w_path;
    wire #(4:14:24) w_net;
    parameter P = (5:15:25);
    parameter Q = (1:2:3) + (10:20:30);
    assign #(1:11:21) w_assign = a;
    buf #(2:12:22) g (w_gate, a);
    u_buf #(3:13:23) u (w_udp, a);
    assign w_net = a;
    assign #P w_param = a;
    path_buf p (w_path, a);

    initial begin
        $display("Q=%0d", Q);
        $monitor("%0t assign=%b gate=%b udp=%b net=%b param=%b path=%b",
                 $time, w_assign, w_gate, w_udp, w_net, w_param, w_path);
        d1 = 8'd1; d2 = 8'd2; d3 = 8'd3;
        a = 0;
        #40 a = 1;
        #40 $monitoroff;
        #(d1:d2:d3) $display("%0t runtime", $time);
        #(1:2:3) $display("%0t constant", $time);
        $finish(0);
    end
endmodule
