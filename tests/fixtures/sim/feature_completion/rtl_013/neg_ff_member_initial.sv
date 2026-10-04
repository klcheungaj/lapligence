// IEEE 1800-2009 9.2.2.4: a record member written by always_ff has no other writer.
module tb;
  typedef struct { logic [3:0] a; logic [3:0] b; } s_t;
  s_t s; logic c; logic [3:0] d;
  always_ff @(posedge c) s.a <= d;
  initial s.a = 1;
  initial $finish(0);
endmodule
