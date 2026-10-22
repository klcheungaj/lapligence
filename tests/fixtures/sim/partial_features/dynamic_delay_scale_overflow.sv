// An integral delay is a 64-bit time value whose scaled tick count wraps
// modulo 2^64 (decision S01-D1): 2^64-1 units of 1ns are 2^64-1000 ticks of
// 1ps.
`timescale 1ns/1ps
module tb;
    logic [63:0] delay_value;
    initial begin
        delay_value=64'hffffffffffffffff;
        #delay_value;
        $display("after %0d", $time > 64'd18446744073709000);
        $finish(0);
    end
endmodule
