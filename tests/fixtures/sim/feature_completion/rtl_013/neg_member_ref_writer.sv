// IEEE 1800-2009 9.2.2.4 and 23.3.3.3: a ref port to a record member overlaps its writer.
typedef struct { logic [7:0] a; logic [7:0] b; } s_t;
module w(ref logic [7:0] r, input logic c);
  always_ff @(posedge c) r <= 8'h1;
endmodule
module tb;
  s_t s; logic c;
  w u(.r(s.a), .c(c));
  always_comb s.a = 8'h2;
  initial $finish(0);
endmodule
