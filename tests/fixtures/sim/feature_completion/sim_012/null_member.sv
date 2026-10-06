// SIM-012 A03: reading or writing a member through a null virtual interface
// is a run-time error at the access and the process stops (SV 25.9).
interface ifc;
  int x;
endinterface

module tb;
  ifc a();
  virtual ifc v;
  initial begin
    v = a;
    v.x = 3;
    $display("x=%0d", v.x);
    v = null;
    v.x = 4;
    $display("unreached");
  end
endmodule
