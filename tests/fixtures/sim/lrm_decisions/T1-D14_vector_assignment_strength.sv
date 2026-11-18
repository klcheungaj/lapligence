// Decision T1-D14: a continuous assignment strength applies to a net of any
// width, as industry-standard simulators accept it: to a bit-select, a part
// select or an indexed part-select of a vector net, to a whole vector net
// (wire, wand, tri0), to an element of an unpacked net array and to a bit of
// such an element. Each selected bit is driven with the given strengths and
// resolves with the other drivers of that bit (28.12). Supply nets still take
// no assignment strength.
//
// IEEE 1800-2009 10.3.4 (SystemVerilog-1800-2009.txt L12940-12941): "The
// driving strength of a continuous assignment can be specified by the user.
// This applies only to assignments to scalar nets, except for nets of types
// supply0 and supply1." L12945-12946: "Whenever the continuous assignment
// drives the net, the strength of the value shall be simulated as specified."
module tb;
  reg a, b, c, d;
  reg [3:0] e;
  wire [3:0] v;
  wire [3:0] u;
  wire [3:0] whole;
  wand [1:0] wa;
  tri0 [1:0] t;
  wire w [2];
  wire [1:0] m [2];
  assign (weak0, weak1) v[0] = a;
  assign v[0] = b;
  assign (weak0, weak1) v[2:1] = {a, c};
  assign (pull0, pull1) v[1] = d;
  assign (weak0, weak1) u[1 +: 2] = {c, a};
  assign (weak0, weak1) u[0] = a;
  assign (weak0, weak1) u[0] = c;
  assign (supply0, supply1) u[3] = a;
  assign (weak0, weak1) u[3] = c;
  assign (weak0, weak1) whole = e;
  assign whole[1] = b;
  assign (strong0, strong1) wa = e[1:0];
  assign (pull0, pull1) t = e[3:2];
  assign (weak0, weak1) w[0] = a;
  assign w[0] = d;
  assign (strong0, weak1) m[1][0] = a;
  initial begin
    a = 1; b = 1'bz; c = 0; d = 1'bz; e = 4'b1010;
    #1 $display("1 v=%b %v %v %v %v u=%b %v %v %v %v", v, v[3], v[2], v[1], v[0], u, u[3], u[2], u[1], u[0]);
    $display("1 whole=%b %v %v wa=%b %v t=%b %v w0=%v m1=%b %v", whole, whole[3], whole[1], wa, wa[0], t, t[1], w[0], m[1], m[1][0]);
    b = 0; d = 0; c = 1; a = 0;
    #1 $display("2 v=%b %v %v %v %v u=%b %v %v %v %v", v, v[3], v[2], v[1], v[0], u, u[3], u[2], u[1], u[0]);
    $display("2 whole=%b %v %v w0=%v m1=%b %v", whole, whole[3], whole[1], w[0], m[1], m[1][0]);
    $finish;
  end
endmodule
