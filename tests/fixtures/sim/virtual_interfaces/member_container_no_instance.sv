// The design has no instance of `ifc` (25.9 L44893-44895: a null virtual
// interface "has the value null before it is initialized. Attempting to use
// a null virtual interface shall result in a fatal run-time error"). A
// `foreach` over a fixed member reads only its shape; a task that pushes onto
// a queue member through its handle formal ends the run inside the task.
interface ifc;
  int a[4];
  int q[$];
endinterface

module tb;
  virtual ifc v;
  int x;
  task automatic put(virtual ifc h);
    h.q.push_back(3);
  endtask
  initial begin
    $display("start %0d", (v == null) ? 0 : v.q.sum());
    foreach (v.a[i]) x += i;
    $display("foreach %0d", x);
    put(v);
    $display("not reached");
  end
endmodule
