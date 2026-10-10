// SIM-032 A03: programs bound into every instance of a module type, into a
// list of instances and into one instance from inside the design
// (IEEE 1800-2009 23.11, 24.3).
program chk(input logic [3:0] v);
  initial #(v) $display("%m v=%0d t=%0d", v, $time);
endprogram

program tag(input logic [3:0] v);
  initial #(v + 10) $display("%m tagged v=%0d t=%0d", v, $time);
endprogram

module dut(input logic [3:0] x);
  logic [3:0] v;
  assign v = x + 4'd1;
endmodule

module tb;
  dut u0(.x(4'd1));
  dut u1(.x(4'd2));
  dut u2(.x(4'd3));
  bind u1 tag t1(.v(v));
endmodule

bind dut chk c(.v(v));
bind dut: tb.u0, tb.u2 tag t(.v(v));
