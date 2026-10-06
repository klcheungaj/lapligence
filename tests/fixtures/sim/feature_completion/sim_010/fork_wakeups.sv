// SIM-010: event controls and waits on automatics that another process of
// the same fork writes wake on each change (SV 9.4.2, 9.4.3): task locals
// shared with detached branches (bare, edge expression, real, level wait),
// a block automatic written by the parent, one written by a joined sibling,
// and a store of an unchanged value, which is no event.
module tb;
  logic level = 1;

  task automatic bare();
    logic l = 0;
    fork #2 l = 1; join_none
    @(l);
    $display("bare %0d", $time);
  endtask

  task automatic edge_expr();
    logic idle = 0;
    fork #2 idle = 1; join_none
    @(posedge (level & idle));
    $display("edge %0d", $time);
  endtask

  task automatic real_change();
    real r = 0.5;
    fork #1 r = 2.5; join_none
    @(r);
    $display("real %0.1f %0d", r, $time);
  endtask

  task automatic level_wait();
    int x = 0;
    fork #1 x = 1; #2 x = 2; join_none
    wait (x == 2);
    $display("wait %0d %0d", x, $time);
  endtask

  initial begin
    automatic int y = 0;
    automatic logic b = 0;
    bare();
    edge_expr();
    real_change();
    level_wait();
    fork
      begin @(b); $display("branch %0d", $time); end
    join_none
    #1 b = 1;
    fork
      begin wait (y == 2); $display("sibling %0d", $time); end
      begin #1 y = 1; #1 y = 2; end
    join
    fork
      begin @(y); $display("changed y=%0d %0d", y, $time); end
    join_none
    #1 y = 2;
    #1 y = 3;
    #1 $finish;
  end
endmodule
