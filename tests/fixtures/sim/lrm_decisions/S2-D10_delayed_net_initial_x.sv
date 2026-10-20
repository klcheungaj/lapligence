// Decision S2-D10: a net with a net delay and at least one driver starts at
// x, the output of its drivers before they first evaluate, and reaches its
// resolved value through its delay: x->0 takes the fall delay, x->1 the rise
// delay and x->z the turn-off delay. Every bit of a driven vector starts at
// x, and the strength reads StX. A net without a driver keeps its default:
// z, 0 for tri0 and 1 for tri1. This is the
// industry-standard behaviour; llg previously started a delayed net at its
// default value.
//
// IEEE 1800-2009 6.7 (SystemVerilog-1800-2009.txt L5199-5200): "The default
// initialization value for a net shall be the value z. Nets with drivers
// shall assume the output value of their drivers." 28.16 (L48369-48370):
// "Net delays refer to the time it takes from any driver on the net changing
// value to the time when the net value is updated and propagated further."
`timescale 1ns/1ns
module sub(output o);
  assign o = 1'b0;
endmodule

module tb;
  reg a;
  wire #(2,3,4) nodrv;
  wire #(2,3,4) wz;
  wire #(2,3,4) wa;
  wire #(2,3,4) w1;
  tri0 #(2,3,4) t0n;
  tri0 #(2,3,4) t0z;
  tri1 #(2,3,4) t1z;
  wand #(2,3,4) wan;
  wor #(2,3,4) wo;
  wire #(2,3,4) pu;
  wire #(2,3,4) g;
  wire #(2,3,4) gz;
  wire #(2,3,4) p;
  wire [3:0] #(2,3,4) v;
  wire #(2,3,4) na [0:1];
  assign wz = 1'bz;
  assign wa = a;
  assign w1 = 1'b1;
  assign t0z = 1'bz;
  assign t1z = 1'bz;
  assign wan = a;
  assign wan = 1'b1;
  assign wo = a;
  assign wo = 1'b0;
  pullup (pu);
  buf (g, a);
  bufif1 (gz, a, 1'b0);
  sub u (.o(p));
  assign v = {a, 1'b1, 1'bz, 1'b0};
  assign na[0] = a;
  initial begin
    $monitor("%0t nodrv=%b wz=%b wa=%b w1=%v t0n=%b t0z=%b t1z=%b wan=%b wo=%b pu=%b g=%b gz=%b p=%b v=%b na=%b%b",
             $time, nodrv, wz, wa, w1, t0n, t0z, t1z, wan, wo, pu, g, gz, p, v, na[0], na[1]);
    a = 1'b0;
    #10 $finish;
  end
endmodule
