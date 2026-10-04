// SIM-003: repeated construction, recursion, suspension and cancellation of
// native record activations. IEEE 1800-2009 6.21, 9.6.2, 13.3-13.5.
`timescale 1ns/1ns
module tb;
  typedef struct {string s; real r;} leaf_t;
  typedef struct {leaf_t l; string tags [0:3]; int n;} node_t;

  node_t keep;
  int total, ticks;

  function automatic node_t make(input int n);
    node_t v;
    v.l.s = $sformatf("n%0d", n);
    v.l.r = real'(n);
    v.tags[0] = v.l.s;
    v.tags[3] = {v.l.s, "!"};
    v.n = n;
    return v;
  endfunction

  // Each activation owns two records; the result reads both after the
  // recursive call returns.
  function automatic int churn(input node_t v, input int k);
    node_t a, b;
    a = v;
    b = make(k);
    if (k == 0) return a.n + b.n;
    return churn(b, k - 1) + a.tags[3].len();
  endfunction

  task automatic slow(input node_t v, output node_t o);
    node_t w;
    w = v;
    #10;
    w.tags[1] = "slow";
    o = w;
  endtask

  // A block-local record is created for every iteration across suspension.
  task automatic ticker(input int count, output int last);
    node_t acc;
    acc.n = 0;
    for (int k = 0; k < count; k++) begin
      node_t step;
      step = make(acc.n + 1);
      #1;
      acc.n = step.n;
      acc.tags[1] = step.l.s;
    end
    last = acc.n * 100 + acc.tags[1].len();
  endtask

  initial begin
    for (int i = 0; i < 2000; i++) total += churn(make(i), 2);
    $display("total %0d", total);
    fork
      begin : run
        slow(make(5), keep);
      end
      begin
        #3 disable run;
      end
    join
    $display("cancelled [%s] %0d %0t", keep.l.s, keep.n, $time);
    slow(make(6), keep);
    $display("finished %s %s %s %0d %0t", keep.l.s, keep.tags[1], keep.tags[3], keep.n, $time);
    ticker(25, ticks);
    $display("ticks %0d %0t", ticks, $time);
    $finish(0);
  end
endmodule
