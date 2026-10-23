// Decision T0-D3, several drivers: each bit of a driven net holds X until a
// driver that writes that bit first evaluates. A driver of some bits does
// not expose the other drivers' bits as z before those drivers evaluate, so
// a bit whose own driver's first value is X makes no time-0 change, whatever
// the order in which the drivers of the vector evaluate. This holds for
// bits driven by separate continuous assignments, by module path outputs
// and next to a delayed gate.
//
// IEEE 1800-2009 6.7 (SystemVerilog-1800-2009.txt L5199-5200):
//   "The default initialization value for a net shall be the value z. Nets
//   with drivers shall assume the output value of their drivers."
// IEEE 1800-2009 4.9.1 (L3509-3511):
//   "A continuous assignment process is also evaluated at time 0 in order to
//   propagate constant values. This includes implicit continuous assignments
//   inferred from port connections (see 4.9.6)."
// IEEE 1800-2009 9.4.2 (L11766-11768):
//   "The execution of a procedural statement can be synchronized with a value
//   change on a net or variable or the occurrence of a declared event."
//
// `a` is a 2-state variable without an initializer, so it starts at 0 and
// no simulator sees a time-0 change on it (see T0-D1); `r` stays X.
`timescale 1ns/1ns
module path_buf (input a, output y);
  buf b0 (y, a);
  specify
    (a => y) = (1, 1);
  endspecify
endmodule

module tb;
  bit a;
  logic r;
  wire [3:0] y;
  wire [3:0] w;
  wire [2:0] m;
  int ev_y1, ev_y3, ev_w1, ev_w3, ev_m2;
  path_buf u0 (a, y[0]);
  path_buf u1 (a, y[1]);
  path_buf u2 (a, y[2]);
  path_buf u3 (a, y[3]);
  assign w[0] = r;
  assign w[1] = r;
  assign w[3] = r;
  buf #2 (m[0], a);
  assign m[1] = a;
  assign m[2] = r;
  always @(y[1]) ev_y1 = ev_y1 + 1;
  always @(y[3]) ev_y3 = ev_y3 + 1;
  always @(w[1]) ev_w1 = ev_w1 + 1;
  always @(w[3]) ev_w3 = ev_w3 + 1;
  always @(m[2]) ev_m2 = ev_m2 + 1;
  initial begin
    #3 $display("t3 y=%b w=%b m=%b events y1=%0d y3=%0d w1=%0d w3=%0d m2=%0d",
                y, w, m, ev_y1, ev_y3, ev_w1, ev_w3, ev_m2);
    a = 1'b1;
    #3 $display("t6 y=%b m=%b events y1=%0d y3=%0d", y, m, ev_y1, ev_y3);
    $finish;
  end
endmodule
