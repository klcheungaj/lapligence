// llg-test-fixture: tests/fixtures/sim/feature_completion/adv_002/pulse_widths.v
// IEEE 1364-2001 6.1.3, 7.14; IEEE 1800-2009 10.3.3, 28.16. Continuous,
// gate, UDP and net delays are inertial: an input pulse narrower than the
// delay is descheduled (steps b and c), a pulse at least as wide propagates,
// and a change to a new value replaces the pending one. Stimulus uses
// nonblocking assignments so a maturing Active update at the same time
// precedes the input change (IEEE 1800-2009 4.4.2): a pulse exactly as wide
// as the delay propagates.
`timescale 1ns/1ns
primitive buf_udp (out, in);
    output out;
    input in;
    table
        0 : 0;
        1 : 1;
    endtable
endprimitive

module tb;
    reg a;
    wire yc, yg, yu;
    wire #4 yn;
    assign #4 yc = a;
    buf #4 g (yg, a);
    buf_udp #(4, 4) u (yu, a);
    assign yn = a;

    initial begin
        $monitor("%0t yc=%b yg=%b yu=%b yn=%b", $time, yc, yg, yu, yn);
        a <= 1'b0;
        #10 a <= 1'b1;
        #3 a <= 1'b0;
        #7 a <= 1'b1;
        #4 a <= 1'b0;
        #6 a <= 1'b1;
        #5 a <= 1'b0;
        #15 a <= 1'b1;
        #2 a <= 1'bx;
        #8 a <= 1'b0;
        #10 $finish(0);
    end
endmodule
