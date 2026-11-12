// FIN-001 R2 negative: a task waiting on a record `ref` formal bound to a whole
// record property with a string member. The property's value publishes no
// change the formal follows, so the call rejects at its location rather than
// waiting forever (SIM-009). Run with `-D N=3` and `-D N=16`.
`ifndef N
`define N 3
`endif
typedef struct { int a; string s; int arr[`N]; } rec_t;
class H;
  rec_t r;
endclass
module tb;
  H h = new;
  task automatic w(ref rec_t x);
    @(x.a);
    $display("woke %0d", x.a);
  endtask
  initial begin
    fork
      w(h.r);
      #1 h.r.a = 1;
    join
  end
endmodule
