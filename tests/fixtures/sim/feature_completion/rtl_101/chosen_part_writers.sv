// Continuous assignments and procedures writing different bits of one member
// of an element of a record's member array: each has its own longest static
// prefix, so none conflicts (SV 6.5, 11.5.3).
module tb;
  typedef struct { logic [7:0] a; logic [7:0] b; } e_t;
  typedef struct { e_t in [16]; logic [7:0] c; } s_t;
  s_t s;
  logic [7:0] x, z;
  assign s.in[1].a[3:0] = x[3:0];
  assign s.in[1].a[7:4] = z[7:4];
  always_comb s.in[2].a[3:0] = x[7:4];
  always_comb s.in[2].a[7:4] = z[3:0];
  initial begin
    x = 8'h34;
    z = 8'hf1;
    #1 $display("%h %h", s.in[1].a, s.in[2].a);
    x = 8'hc5;
    #1 $display("%h %h", s.in[1].a, s.in[2].a);
    $finish;
  end
endmodule
