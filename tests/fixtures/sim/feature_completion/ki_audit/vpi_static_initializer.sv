// Known issue (docs/known_issues.md, VPI work during static initialization):
// $sched (vpi_static_initializer.c) is called from a static declaration
// initializer, which the model runs before the VPI application's startup
// routines register it. The call stops the run with a VPI error at time 0,
// so nothing is printed. 13.4.4 also asks for an error once
// the call runs, since its calltf schedules a delayed put (S13-D5).
`timescale 1ns/1ns
module tb;
  int x;
  int y = $sched();
  initial #2 $display("x=%0d y=%0d", x, y);
endmodule
