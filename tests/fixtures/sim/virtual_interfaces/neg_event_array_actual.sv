interface ifc;
  event ea[2];
endinterface

module tb;
  ifc i();
  virtual ifc v = i;
  task automatic rebind(ref event x);
    x = null;
  endtask
  initial rebind(v.ea[1]);
endmodule
