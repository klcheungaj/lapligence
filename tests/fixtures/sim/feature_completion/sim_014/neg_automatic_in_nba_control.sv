// SIM-014 A03 negative: automatic variables shall not be referenced in
// intra-assignment event controls of nonblocking assignments (SV 13.3.2).
module tb;
  logic y;
  task automatic t();
    logic w;
    y <= @(w) 1'b1;
  endtask
  initial t();
endmodule
