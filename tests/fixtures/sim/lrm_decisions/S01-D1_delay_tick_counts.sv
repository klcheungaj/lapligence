// Decision S01-D1: how a delay value becomes a 64-bit count of simulation
// ticks. A real delay whose tick count has no 64-bit value (infinite, NaN,
// or too large once rounded and scaled to the design precision) waits one
// tick; the simulator may warn. An integral delay keeps its low 64 bits,
// like a time variable, and its count of design ticks wraps modulo 2^64.
// Here 1 tick is 1ps and the delays are in ns.
//
// IEEE 1800-2009 9.4.1 (SystemVerilog-1800-2009.txt L11747-11751): "A
// procedural statement following the delay control shall be delayed in its
// execution with respect to the procedural statement preceding the delay
// control by the specified delay. If the delay expression evaluates to an
// unknown or high-impedance value, it shall be interpreted as zero delay. If
// the delay expression evaluates to a negative value, it shall be
// interpreted as a twos-complement unsigned integer of the same size as a
// time variable." The text does not cover a delay that has no 64-bit tick
// count; llg follows industry-standard behaviour.
`timescale 1ns/1ps
module tb;
  real d;
  logic [128:0] wide;
  logic [63:0] big;
  initial begin
    $timeformat(-12, 0, "", 0);
    d = 1.0e308;
    d = d * 10.0;
    #(d);
    $display("infinite: %t", $realtime);
    d = $bitstoreal(64'h7FF8_0000_0000_0000);
    #(d);
    $display("NaN: %t", $realtime);
    d = 1.0e20;
    #(d);
    $display("too large: %t", $realtime);
    d = 1.0e7;
    #(d);
    $display("in range: %t", $realtime);
    wide = (129'd1 << 100) | 129'd5;
    #(wide);
    $display("wide: %t", $realtime);
    big = 64'h0050_0000_0000_0000;
    #(big);
    $display("wrapped: %0d ns", $time);
    $finish;
  end
endmodule
