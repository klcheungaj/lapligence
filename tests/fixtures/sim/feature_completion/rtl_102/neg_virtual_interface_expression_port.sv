// Deferred: a modport expression port read through a virtual interface handle.
interface bus_if;
  logic [7:0] a;
  modport m (input .p(a[3:0]));
endinterface
module tb;
  bus_if bi();
  virtual bus_if.m v;
  initial begin
    v = bi;
    bi.a = 8'h5c;
    #1 $display("%h", v.p);
    $finish(0);
  end
endmodule
