// IEEE 1800-2009 10.9.1: a replicated pattern whose extent exceeds the
// destination fixed array is rejected as a shape error.
module tb;
    logic [7:0] values [0:1];

    initial begin
        values = '{3{8'h2a}};
        $finish(0);
    end
endmodule
