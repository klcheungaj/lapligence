module tb;
  reg clk;
  initial begin
    clk = 1'b0;
    $dumpports(tb, "ports.evcd");
    #1 clk = 1'b1;
    $dumpportsoff;
    $dumpportson;
    $dumpportsall;
    $dumpportslimit(4096, "ports.evcd");
    $dumpportsflush;
    $finish;
  end
endmodule
