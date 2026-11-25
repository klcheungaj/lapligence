interface ifc;
  event ea[2];
endinterface
module tb;
  ifc i();
  virtual ifc v = i;
  initial -> v.ea[1];
endmodule
