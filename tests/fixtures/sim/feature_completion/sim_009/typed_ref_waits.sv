// SIM-009: a task that waits directly on a whole `ref` formal (`@(posedge
// s)`, `@(x)`, `wait (v >= 3)`) takes the typed call path for a caller
// automatic actual, following the variable its descriptor names, so it can
// recurse and take a native record formal (SV 9.4.2, 9.4.3, 13.5.2). Module
// signals keep their specializations and element actuals the expansion.
module tb;
  typedef struct { string name; int n; } rec_t;
  logic g = 0;
  logic mem [2];

  task automatic r(ref logic s, input int n);
    @(posedge s);
    $display("r %0d at %0d", n, $time);
    if (n > 0) r(s, n - 1);
  endtask

  task automatic lvl(ref int v, input rec_t tag);
    wait (v >= 3);
    $display("lvl %s %0d at %0d", tag.name, v, $time);
  endtask

  task automatic rl(ref real x);
    @(x);
    $display("real %0.2f at %0d", x, $time);
  endtask

  initial begin
    automatic logic l = 0;
    automatic int c = 0;
    automatic real x = 0.0;
    fork
      repeat (3) begin #1 l = 1; #1 l = 0; end
      begin #10 c = 1; #1 c = 3; end
      begin #20 x = 2.5; end
    join_none
    r(l, 2);
    lvl(c, '{"t", 1});
    rl(x);
    fork begin #1 g = 1; #1 mem[1] = 1; end join_none
    r(g, 0);
    r(mem[1], 0);
    $finish;
  end
endmodule
