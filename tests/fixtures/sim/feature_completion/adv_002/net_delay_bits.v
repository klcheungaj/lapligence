// llg-test-fixture: tests/fixtures/sim/feature_completion/adv_002/net_delay_bits.v
// IEEE 1364-2001 6.1.3, 7.14; IEEE 1800-2009 10.3.3, 28.16. A net delay
// delays every change applied to the net, after resolution. 6.1.3 contrasts
// a declaration assignment, whose "rising and falling delays shall not be
// applied to the individual bits", with the net delay, but states no rule for
// the net delay: the vector is scheduled and filtered as one, and the new
// value of its least significant bit selects the delay (S2-D3), the
// industry-standard behaviour. A scalar net follows the from/to table.
`timescale 1ns/1ns
module tb;
    reg [1:0] a;
    reg s;
    wire [1:0] #(10,5) w;
    wire [1:0] #4 p;
    wire #(5,7,3) sc;
    assign w = a;
    assign p = a;
    assign sc = s;

    initial begin
        $monitor("%0t w=%b p=%b sc=%b", $time, w, p, sc);
        a = 2'b01;
        s = 1'b0;
        #20 a = 2'b10;
        #20 a = 2'b11;
        #2 a = 2'b01;
        #18 s = 1'bx;
        #10 s = 1'bz;
        #10 s = 1'b1;
        #10 a = 2'b00;
        #2 a = 2'b01;
        #8 $finish(0);
    end
endmodule
