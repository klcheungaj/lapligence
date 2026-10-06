`include "child_defs.svh"
module child #(parameter int W = 4) (output logic [W-1:0] y);
  leaf #(.VALUE(`CHILD_VALUE)) u_leaf(.y(y));
endmodule
