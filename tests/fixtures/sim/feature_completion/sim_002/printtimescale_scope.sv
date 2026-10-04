// SIM-002: $printtimescale reports the named instance or `$unit`, and the
// current scope's module, package or `$unit` without an operand.
// SV2009 20.4.1 (Syntax 20-3, output format), 3.14.2.3; FND-002 L-F10-11-01.
timeunit 100ps;
timeprecision 10ps;
function automatic void unit_show();
  $printtimescale;
endfunction
`timescale 1ns/1ps
module child;
  task automatic show();
    $printtimescale;
  endtask
endmodule
`timescale 10ns/100ps
module leaf;
endmodule
`timescale 100us/10us
interface bus_if;
  task automatic show();
    $printtimescale;
  endtask
endinterface
`timescale 1ms/1us
program prog;
endprogram
`timescale 10fs/1fs
package pkg;
  function automatic void show();
    $printtimescale;
  endfunction
endpackage
`timescale 1us/1ns
module tb;
  child c();
  leaf arr[0:2]();
  if (1) begin : g
    leaf inner();
  end
  bus_if bus();
  prog p();
  initial begin : main
    $printtimescale(c);
    $printtimescale;
    $printtimescale(tb);
    $printtimescale(tb.c);
    $printtimescale(arr[2]);
    $printtimescale(g.inner);
    $printtimescale(bus);
    $printtimescale(p);
    $printtimescale($unit);
    c.show();
    bus.show();
    pkg::show();
    unit_show();
    $finish(0);
  end
endmodule
