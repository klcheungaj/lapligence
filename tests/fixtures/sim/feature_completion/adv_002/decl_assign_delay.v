// llg-test-fixture: tests/fixtures/sim/feature_completion/adv_002/decl_assign_delay.v
// IEEE 1364-2001 6.1.3; IEEE 1800-2009 10.3.3. "When there is a continuous
// assignment in a declaration, the delay is part of the continuous
// assignment and is not a net delay. Thus, it shall not be added to the
// delay of other drivers on the net. Furthermore, if the assignment is to a
// vector net, then the rising and falling delays shall not be applied to the
// individual bits." The second driver of w is undelayed; vw takes one delay.
`timescale 1ns/1ns
module tb;
    reg b, c;
    reg [1:0] v;
    wire #(4,6) w = b;
    assign w = c;
    wire [1:0] #(10,5) vw = v;

    initial begin
        $monitor("%0t w=%b vw=%b", $time, w, vw);
        b = 1'bz;
        c = 1'b0;
        v = 2'b01;
        #20 c = 1'b1;
        #20 b = 1'b1;
        c = 1'bz;
        v = 2'b10;
        #20 $finish(0);
    end
endmodule
