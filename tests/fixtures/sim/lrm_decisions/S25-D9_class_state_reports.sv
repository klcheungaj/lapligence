// S25-D9: `$strobe`, `$monitor` and the file variants may name class properties,
// `this` and a report may be issued in a class method. The report reads the
// property at the end of the slot through the handle as it stands then.
// 13.3.2: variables of automatic tasks "shall not be traced with system tasks
// such as $monitor and $dumpvars." A class property is neither: it lives in its
// object.
// 21.2.2: "The action shall occur just before simulation time is advanced and
// after all other events at that time have occurred so that the data written
// are sure to be the correct data for that simulation time."
// 21.2.3: "each time a variable or an expression in the argument list changes
// value [...] the entire argument list is displayed at the end of the time step".
// 8.4: "Accessing nonstatic members (see 8.8) or virtual methods (see 8.19) via
// a null object handle is illegal. The result of an illegal access via a null
// object is indeterminate, and implementations may issue an error."
// The decision: a monitor reports when the value it prints changes, also when
// a handle on its path is rebound. A null handle on the path when the report
// evaluates stops the run as any null access does (S99-D8); a handle that is
// null at the call and set later in the slot is not an access. A report in a
// method keeps its object.
// Some simulators reject a class property in `$monitor` or in a report in a
// method, and print a property read through a module handle as it stood at the
// `$strobe` call; llg follows the clauses.
`timescale 1ns/1ns
module tb;
  class N;
    int v;
    N next;
    function new(int x);
      v = x;
    endfunction
    function void show();
      $strobe("show %0d", v);
    endfunction
  endclass
  N h, a, b;
  initial begin
    a = new(1);
    b = new(2);
    a.next = b;
    // The handle is null at the call and set before the slot ends.
    $strobe("strobe %0d", h.v);
    h = a;
    h.v = 3;
    #1;
    $monitor("monitor %0d %0d", h.v, h.next.v);
    #1 h.v = 4;
    #1 h = b;
    b.next = a;
    #1 h = new(2);
    h.next = a;
    #1 a.v = 5;
    #1 h.next = b;
    #1 a.v = 6;
    #1 begin
      a.show();
      a.v = 7;
      a = null;
    end
    #1 $finish(0);
  end
endmodule
