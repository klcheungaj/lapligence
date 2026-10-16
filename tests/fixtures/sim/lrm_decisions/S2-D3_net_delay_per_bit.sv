// Decision S2-D3: a net delay on a vector net delays each bit on its own: a
// bit takes its own rise/fall/turn-off delay from its own transition and its
// pending update is kept, replaced or descheduled independently of the other
// bits. llg previously scheduled one update for the whole net at the smallest
// delay of the changed bits.
//
// IEEE 1800-2009 10.3.3 (SystemVerilog-1800-2009.txt L12911-12915): "This
// syntax, called a net delay, means that any value change that is to be
// applied to wireA by some other statement shall be delayed for ten time units
// before it takes effect. When there is a continuous assignment in a
// declaration, the delay is part of the continuous assignment and is not a net
// delay. ... Furthermore, if the assignment is to a vector net, then the
// rising and falling delays shall not be applied to the individual bits if the
// assignment is included in the declaration." The contrast implies that a net
// delay applies its rise and fall delays to the individual bits. 28.16
// (L48369-48370): "Net delays refer to the time it takes from any driver on
// the net changing value to the time when the net value is updated and
// propagated further."
`timescale 1ns/1ns
module tb;
  reg [1:0] a;
  wire [1:0] #(10,5) w;
  wire [1:0] #4 p;
  assign w = a;
  assign p = a;
  initial begin
    $monitor("%0t w=%b p=%b", $time, w, p);
    a = 2'b01;
    #20 a = 2'b10;
    #20 a = 2'b11;
    #2 a = 2'b01;
    #18 $finish;
  end
endmodule
