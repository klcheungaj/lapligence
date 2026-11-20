// $sched (vpi_static_initializer.c) is called from a static declaration
// initializer and runs: the VPI application starts before static
// initialization (decision S43-D12). Known issue (docs/known_issues.md, VPI
// work during static initialization): its calltf schedules a delayed put,
// for which 13.4.4 asks for an error (S13-D5); the put matures at time 1
// instead, so the run prints x=5 y=7.
`timescale 1ns/1ns
module tb;
  int x;
  int y = $sched();
  initial #2 $display("x=%0d y=%0d", x, y);
endmodule
