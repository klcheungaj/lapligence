// A two-dimensional array of interface instances as a value.
interface ifc;
  int n;
endinterface

module tb;
  ifc i[2][2]();
  virtual ifc v[2][2];
  initial begin
    v = i;
    $display("%0d", v[0][0].n);
    $finish(0);
  end
endmodule
