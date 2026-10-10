// SIM-009-A03: an event list that names an event next to an expression over a
// `ref` formal, an `iff`-qualified edge of a selected bit, and implicit `@*`
// sensitivity reading a `ref` formal are evaluated by the waiting process on
// the typed call path, so the tasks recurse (SV 9.4.2, 9.4.2.1, 9.4.2.2,
// 9.4.2.3, 13.5.2).
module tb;
  event ev, ev2;

  task automatic either(ref int r, input string tag, input int n);
    @(ev or (r > n));
    $display("%s %0d r %0d at %0d", tag, n, r, $time);
    if (n > 0) either(r, tag, n - 1);
  endtask

  task automatic gated(ref logic [3:0] r, input int n);
    @(ev2 or posedge r[0] iff r > 4'd2);
    $display("gated %0d r %0d at %0d", n, r, $time);
    if (n > 0) gated(r, n - 1);
  endtask

  task automatic implicit_list(ref int r, input int n);
    @* if (r > 0) $display("implicit %0d sees %0d at %0d", n, r, $time);
    if (n > 0) implicit_list(r, n - 1);
  endtask

  initial begin
    automatic int loc = 0;
    automatic logic [3:0] bits = 0;
    automatic int imp = 0;
    fork
      either(loc, "either", 2);
      gated(bits, 1);
      implicit_list(imp, 1);
    join_none
    #1 -> ev;
    #1 loc = 5;
    #1 loc = 1;
    #1 -> ev2;
    #1 bits = 4'd1;
    #1 bits = 4'd2;
    #1 bits = 4'd3;
    #1 imp = 1;
    #1 imp = 2;
    #1 -> ev;
    #1 $finish;
  end
endmodule
