// FIN-001 R2 negative: a task waiting on a `ref real` formal bound to a real
// member of a record property with a string member. A direct store into the
// member publishes no change the formal follows, so the call rejects at its
// location rather than waiting forever (SV 9.4.2). Run with `-D N=3` and
// `-D N=16`.
`ifndef N
`define N 3
`endif
typedef struct { int a; string s; real rv; int arr[`N]; } rec_t;
class H;
  rec_t r;
endclass
module tb;
  H h = new;
  task automatic w(ref real x);
    @(x);
    $display("woke %0g", x);
  endtask
  initial begin
    fork
      w(h.r.rv);
      #1 h.r.rv = 2.5;
    join
  end
endmodule
