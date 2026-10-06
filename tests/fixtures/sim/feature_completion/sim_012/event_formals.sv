// SIM-012 A02: event formals of every direction pass through virtual
// interface dispatch with their identity: an input names the caller's event,
// an output and an inout return the interface's event, a ref triggers the
// caller's event (SV 25.9, 13.5, 15.5).
interface ifc;
  event done;
  task automatic waiter(input event e, output event o); @(e); o = done; endtask
  task automatic fire(ref event e); #1 -> e; endtask
  task automatic swap(inout event e); #1 e = done; endtask
endinterface

module tb;
  ifc a(), b();
  virtual ifc v;
  event go, got, h;
  initial begin
    v = a;
    fork
      begin v.waiter(go, got); $display("waiter %0d", $time); end
      begin v.fire(go); $display("fired %0d", $time); end
    join
    fork
      begin @(got); $display("got a.done %0d", $time); end
      begin #1 -> a.done; end
    join
    v = b;
    h = go;
    v.swap(h);
    fork
      begin @(h); $display("h is b.done %0d", $time); end
      begin #1 -> a.done; #1 -> b.done; end
    join
    $finish;
  end
endmodule
