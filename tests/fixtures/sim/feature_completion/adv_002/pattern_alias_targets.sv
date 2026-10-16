// llg-test-fixture: tests/fixtures/sim/feature_completion/adv_002/pattern_alias_targets.sv
// IEEE 1800-2009 10.3.3, 10.9, 10.11. Each assignment-pattern member is its
// own delayed driver (S2-D7); a member or a whole target split over alias
// groups stays one vector driver whose delay is applied once (S2-D6).
`timescale 1ns/1ns
module tb;
    typedef struct { logic s; logic [1:0] v; logic [1:0] w; logic [1:0] c; } quad_t;
    reg [6:0] a;
    quad_t src;
    wire s;
    wire [1:0] v, w, t;
    wire w0, t1;
    wire [1:0] na [0:1];
    alias w[0] = w0;
    alias t[1] = t1;
    assign src = '{a[6], a[5:4], a[3:2], a[1:0]};
    assign #(10,5) '{s, v, w, na[1]} = src;
    assign #(10,5) t = a[1:0];
    initial begin
        $monitor("%0t s=%b v=%b w=%b w0=%b na1=%b t=%b t1=%b", $time, s, v, w, w0, na[1], t, t1);
        a = 7'b1_01_01_01;
        #20 a = 7'b0_10_10_10;
        #20 a = 7'b0_00_00_00;
        #20 a = 7'b0_01_00_01;
        #2 a = 7'b0_00_00_00;
        #20 $finish(0);
    end
endmodule
