// FIN-001 A3 limit: a deferred action may call a task method (IEEE 1800-2009
// 16.4); a call through a virtual interface handle is rejected at the action.
interface ifc;
  task automatic show(input int x);
    $display("[%0t] show %0d", $time, x);
  endtask
endinterface
module tb;
  ifc i0();
  virtual ifc vif;
  int x = 1;
  initial begin
    vif = i0;
    assert #0 (0) else vif.show(x);
    #1 $finish;
  end
endmodule
