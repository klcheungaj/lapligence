// SIM-012 A03: writing an input modport member through a virtual interface
// view is a frontend error (SV 25.5).
interface ifc;
  int x;
  modport ro(input x);
endinterface

module tb;
  ifc a();
  virtual ifc.ro v;
  initial begin
    v = a;
    v.x = 1;
  end
endmodule
