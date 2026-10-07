// SIM-015: calling a method through a null process handle is a null object
// access (SV 8.4, 9.7); the simulation reports it and ends.
module tb;
  process p;
  initial begin
    p = null;
    $display("before");
    p.await();
    $display("FAIL continued after null await");
  end
endmodule
