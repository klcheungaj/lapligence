// SIM-026 A03 negative: a class handle is not integral, real or string storage.
module tb;
  class C; endclass
  integer c; C o;
  initial begin
    c = $sscanf("1", "%d", o);
    $finish;
  end
endmodule
