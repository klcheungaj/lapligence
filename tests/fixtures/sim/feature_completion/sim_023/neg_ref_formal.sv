// SV 13.3.2: a ref formal of an automatic task is automatic storage.
module tb;
  logic [3:0] v;
  task automatic t(ref logic [3:0] x);
    force x = 4'h1;
  endtask
  initial t(v);
endmodule
