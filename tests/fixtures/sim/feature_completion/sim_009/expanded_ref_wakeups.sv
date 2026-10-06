// SIM-009/SIM-010: a task whose `ref` formal is bound to a caller automatic
// is expanded at the call site; its event control on the formal wakes when
// a fork branch writes the shared actual (SV 9.4.2, 13.5.2, 9.3.2).
module tb;
  task automatic await_edge(ref logic s);
    @(posedge s);
    $display("edge %0d", $time);
  endtask

  task automatic outer();
    logic loc = 0;
    fork #2 loc = 1; join_none
    await_edge(loc);
  endtask

  initial begin
    automatic logic b = 0;
    outer();
    fork #3 b = 1; join_none
    await_edge(b);
    $finish;
  end
endmodule
