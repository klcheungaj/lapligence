interface ifc;
  event ea[2];
endinterface

module tb;
  ifc i();
  virtual ifc v;
  int k = 1;
  initial begin
    -> v.ea[k];
    $display("not here");
  end
  final $display("final");
endmodule
