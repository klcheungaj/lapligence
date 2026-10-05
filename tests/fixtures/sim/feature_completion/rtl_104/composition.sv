// IEEE 1800-2009 11.11 forms composed with processes: a clocked postfix
// increment whose old value is the nonblocking right side, a continuous
// assignment of the registered member, an always_comb comparison of
// overloaded sums, and a task that increments an automatic copy.
module counter(input logic clk, output int seen, output bit flag);
  typedef struct { int v; logic [3:0] tag; } c_t;
  function automatic c_t step(c_t c);
    step.v = c.v + 3;
    step.tag = c.tag + 1;
  endfunction
  function automatic bit below(c_t a, c_t b);
    return a.v < b.v;
  endfunction
  function automatic c_t plus(c_t a, c_t b);
    plus.v = a.v + b.v;
    plus.tag = a.tag ^ b.tag;
  endfunction
  bind ++ function c_t step(c_t);
  bind < function bit below(c_t, c_t);
  bind + function c_t plus(c_t, c_t);
  c_t cnt, q, lim;
  initial begin
    cnt.v = 0;
    cnt.tag = 0;
    q.v = 0;
    q.tag = 0;
    lim.v = 12;
    lim.tag = 0;
  end
  always @(posedge clk) q <= cnt++;
  assign seen = q.v;
  always_comb flag = (cnt + q) < lim;
  task automatic peek(input c_t c, output int old_v, output int new_v);
    c_t old;
    old = c++;
    old_v = old.v;
    new_v = c.v;
  endtask
  always @(negedge clk) begin
    int old_v, new_v;
    peek(cnt, old_v, new_v);
    $display("%0d cnt=%0d/%0d q=%0d/%0d seen=%0d flag=%0d peek=%0d->%0d", $time, cnt.v,
             cnt.tag, q.v, q.tag, seen, flag, old_v, new_v);
  end
endmodule

module tb;
  logic clk = 0;
  int seen;
  bit flag;
  counter dut(.clk(clk), .seen(seen), .flag(flag));
  initial begin
    repeat (6) #5 clk = ~clk;
    #1 $finish(0);
  end
endmodule
