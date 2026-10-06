// SIM-012 A03: a virtual interface accepts only instances of its own
// parameter specialization (SV 25.9).
interface ifc #(parameter W = 4);
  logic [W-1:0] x;
endinterface

module tb;
  ifc #(4) a();
  ifc #(8) b();
  virtual ifc #(4) v;
  initial v = b;
endmodule
