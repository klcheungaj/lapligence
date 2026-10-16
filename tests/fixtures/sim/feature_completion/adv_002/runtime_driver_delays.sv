// llg-test-fixture: tests/fixtures/sim/feature_completion/adv_002/runtime_driver_delays.sv
// IEEE 1800-2009 28.16.1, 10.3.3, 11.12. Continuous-assignment and gate
// delays may be runtime-valued (integer, real, min:typ:max, X); the driver
// evaluates them when it schedules an update, and a change of a delay operand
// alone schedules nothing (S2-D9).
`timescale 1ns/100ps
module tb;
    reg a, en;
    reg [1:0] v;
    integer d, dr, df, dz, d1, d2, d3, dx;
    real rd;
    wire y_single, y_rf, y_real, y_mtm, y_x, y_gate, y_bufif;
    wire [1:0] y_vec;
    assign #d y_single = a;
    assign #(dr, df) y_rf = a;
    assign #(rd) y_real = a;
    assign #(d1:d2:d3) y_mtm = a;
    assign #(dx) y_x = a;
    buf #(d) g_buf (y_gate, a);
    bufif1 #(dr, df, dz) g_bufif (y_bufif, a, en);
    assign #(dr, df) y_vec = v;
    initial begin
        $monitor("%0.1f single=%b rf=%b real=%b mtm=%b x=%b gate=%b bufif=%b vec=%b",
                 $realtime, y_single, y_rf, y_real, y_mtm, y_x, y_gate, y_bufif, y_vec);
        d = 3; dr = 4; df = 2; dz = 6; d1 = 1; d2 = 5; d3 = 9; rd = 2.5;
        a = 0; en = 1; v = 2'b01;
        #10 a = 1;
        #10 d = 1; dr = 7;
        #10 a = 0; v = 2'b10;
        #10 en = 0;
        #10 $finish(0);
    end
endmodule
