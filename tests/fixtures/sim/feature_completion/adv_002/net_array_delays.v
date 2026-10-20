// llg-test-fixture: tests/fixtures/sim/feature_completion/adv_002/net_array_delays.v
// IEEE 1364-2001 6.1.3, 7.14; IEEE 1800-2009 10.3.3, 28.16. A delay in a
// net-array declaration is the net delay of every element: each element
// delays the changes of its resolved value, after a delayed driver's own
// delay, and filters narrower pulses. A vector element is scheduled as one
// vector, a bit-select driver's part on its own (S2-D3).
`timescale 1ns/1ns
module tb;
    reg a;
    reg [1:0] b;
    wire #3 na [0:2];
    wire [1:0] #(4, 2) va [0:1][0:1];
    assign na[0] = a;
    assign #2 na[1] = ~a;
    buf (na[2], a);
    assign va[0][1] = b;
    assign va[1][0][1] = a;

    initial begin
        $monitor("%0t na=%b%b%b va01=%b va10=%b", $time, na[0], na[1], na[2],
                 va[0][1], va[1][0]);
        a = 1'b0;
        b = 2'b01;
        #10 a = 1'b1;
        b = 2'b10;
        #10 a = 1'b0;
        #1 a = 1'b1;
        #19 $finish(0);
    end
endmodule
