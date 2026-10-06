// SIM-012 A03: a timed call keeps the instance it started on when the
// variable is rebound while it waits; a call through a null virtual
// interface is a run-time error at the call and the process stops (SV 25.9).
interface ifc;
  int x;
  task automatic slow(); #5 x = 1; endtask
endinterface

module tb;
  ifc a(), b();
  virtual ifc v;
  initial begin
    v = a;
    fork
      v.slow();
      begin #1 v = b; end
    join
    $display("a.x=%0d b.x=%0d", a.x, b.x);
    v = null;
    v.slow();
    $display("unreached");
  end
endmodule
