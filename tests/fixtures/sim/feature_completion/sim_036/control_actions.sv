// SIM-036 A03: allowed single-call actions run once each, in the Reactive
// region of the step that issued them (IEEE 1800-2009 16.4): display and
// severity system tasks, a void function, a task and a $stop. Under llg's
// non-interactive $stop policy the run continues after the stop report.
module tb;
  int n = 0;
  int v;

  function void note(input int a);
    n += a;
    $display("%0d note a=%0d n=%0d", $time, a, n);
  endfunction

  task tell(input int a);
    $display("%0d tell a=%0d", $time, a);
  endtask

  initial begin
    v = 1;
    assert #0 (1'b1) note(1); else note(100);
    cover #0 (1'b1) note(10);
    assume #0 (1'b0) else note(1000);
    assert #0 (1'b0) else tell(v);
    assert #0 (1'b1) $display("%0d display v=%0d", $time, v);
    assert #0 (1'b0) else $info("info v=%0d", v);
    assert #0 (1'b0) else $warning("warning v=%0d", v);
    assert #0 (1'b0) else $error("error v=%0d", v);
    v = 2;
    $display("%0d issued n=%0d", $time, n);
    #1 $display("%0d n=%0d", $time, n);
    assert #0 (1'b0) else $stop;
    #1 $display("%0d after stop", $time);
    $finish(0);
  end
endmodule
