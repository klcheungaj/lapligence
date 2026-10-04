// SIM-002: $time/$stime/$realtime and %t scale by the unit of the scope that
// contains the call (module, interface, package, `$unit` or class declaring
// scope), whichever scope calls the subroutine. Delays in package and child
// subroutines use the declaring scope's unit and precision.
// SV2009 20.3, 3.14.2.3, 21.2.1.3 (%t), Table 20-3; FND-002 L-F10-11-01.
timeunit 100ps;
timeprecision 10ps;
function automatic void unit_report();
  $display("unit time=%0d realtime=%0.2f", $time, $realtime);
endfunction
`timescale 10us/1us
package pkg;
  function automatic void report(input string who);
    $display("%s: pkg time=%0d stime=%0d realtime=%0.3f t=%0t", who, $time, $stime,
             $realtime, $realtime);
  endfunction
  function automatic void show_value(input time value);
    $display("pkg value t=%0t", value);
  endfunction
  task automatic wait_units();
    #1.5;
  endtask
  class Clock;
    function void show();
      $display("pkg class time=%0d realtime=%0.3f", $time, $realtime);
    endfunction
  endclass
endpackage
`timescale 1ns/1ns
interface bus_if;
  task automatic report();
    $display("bus time=%0d stime=%0d realtime=%0.3f", $time, $stime, $realtime);
  endtask
endinterface
`timescale 100ns/1ns
module child;
  task automatic report();
    $display("child time=%0d realtime=%0.3f t=%0t", $time, $realtime, $realtime);
  endtask
  task automatic relay();
    pkg::report("child");
  endtask
  task automatic delay_one();
    #1;
  endtask
endmodule
`timescale 1us/1ps
module tb;
  class Local;
    function void show();
      $display("tb class time=%0d realtime=%0.6f", $time, $realtime);
    endfunction
  endclass
  child c();
  bus_if bus();
  pkg::Clock package_clock;
  Local local_clock;
  initial begin
    package_clock = new;
    local_clock = new;
    #12.345678;
    $display("tb time=%0d stime=%0d realtime=%0.6f t=%0t", $time, $stime, $realtime, $time);
    unit_report();
    pkg::report("tb");
    c.relay();
    bus.report();
    c.report();
    package_clock.show();
    local_clock.show();
    pkg::show_value($time);
    pkg::wait_units();
    $display("after package delay realtime=%0.6f t=%0t", $realtime, $realtime);
    c.delay_one();
    $display("after child delay realtime=%0.6f t=%0t", $realtime, $realtime);
    $finish(0);
  end
endmodule
