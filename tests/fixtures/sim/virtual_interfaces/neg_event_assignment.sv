interface ifc;
  event ie;
endinterface
module tb;
  ifc i();
  virtual ifc v = i;
  event e;
  initial v.ie = e;
endmodule
