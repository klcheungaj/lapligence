// Decision S2-D3: a net delay on a vector net schedules the net as one
// vector. The net keeps one pending update for its whole value, which a new
// value keeps, replaces or deschedules as a whole, and the new value of the
// least significant bit selects the delay: 0 the fall delay, 1 the rise
// delay, z the turn-off delay and x the smallest of the three. A vector net
// whose drivers cover different bit ranges schedules each separately driven
// part this way on its own. This is the industry-standard behaviour; llg
// previously delayed each bit on its own. The net starts at x (S2-D10). A
// runtime-valued vector net delay selects its delay differently (S2-D9).
//
// IEEE 1800-2009 10.3.3 (SystemVerilog-1800-2009.txt L12911-12915): "This
// syntax, called a net delay, means that any value change that is to be
// applied to wireA by some other statement shall be delayed for ten time units
// before it takes effect. ... Furthermore, if the assignment is to a vector
// net, then the rising and falling delays shall not be applied to the
// individual bits if the assignment is included in the declaration."
// 28.16 (L48369-48370): "Net delays refer to the time it takes from any
// driver on the net changing value to the time when the net value is updated
// and propagated further." 28.16 (L48379-48383): the first delay "refers to
// the transition to the 1 value", the second "to the 0 value", the third "to
// the high-impedance value"; "When a value changes to the unknown (x) value,
// the delay is the smallest of the three delays." The text does not say how
// a vector net delay is selected; the contrast in 10.3.3 suggests per-bit
// delays, but no bit-level rule is stated.
`timescale 1ns/1ns
module tb;
  reg [1:0] a;
  reg [2:0] c;
  reg [3:0] e;
  wire [1:0] #(10,5) w;
  wire [1:0] #4 p;
  // Three delays: the LSB's new value picks rise 10, fall 20 or turn-off 30.
  wire [2:0] #(10,20,30) u;
  // Bits [3:2] have a second driver, so [3:2] and [1:0] are separate parts.
  wire [3:0] #(10,20) sg;
  assign w = a;
  assign p = a;
  assign u = c;
  assign sg = e;
  assign sg[3:2] = 2'bzz;
  initial begin
    $monitor("%0t w=%b p=%b u=%b sg=%b", $time, w, p, u, sg);
    a = 2'b01;
    c = 3'b000;
    e = 4'b0000;
    #20 a = 2'b10;
    #20 a = 2'b11;
    #2 a = 2'b01;
    #18 c = 3'b010;
    e = 4'b0110;
    #40 c = 3'b011;
    #40 c = 3'b01z;
    #40 c = 3'b0x1;
    #40 c = 3'b1x0;
    #40 $finish;
  end
endmodule
