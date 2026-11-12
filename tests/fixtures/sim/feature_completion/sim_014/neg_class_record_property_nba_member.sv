// FIN-001 R2 negative: SV 6.21 forbids nonblocking writes to class
// properties, also to a record property with a string member, whole or one
// member. Run with `-D N=3` and `-D N=16` (both record storage forms).
`ifndef N
`define N 3
`endif
typedef struct { int a; string s; int arr[`N]; } rec_t;
class C;
  rec_t r;
endclass
module tb;
  C c = new;
  rec_t v;
  event e;
  initial begin
    #1 c.r.a <= #1 6;
    #2 $display("%0d", c.r.a);
  end
endmodule
