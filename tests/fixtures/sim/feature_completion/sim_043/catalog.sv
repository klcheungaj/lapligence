// SIM-043: a model emits its VPI object catalog only when an application
// can use it (--vpi, --dpi-lib or user system task call sites). Without it,
// a run-time application still gets callbacks but finds no objects.
`timescale 1ns / 1ns
module tb;
  logic [3:0] v = 4'd0;
  initial begin
    #2 v = 4'd9;
    #1 $display("[%0t] hdl: v=%0d", $time, v);
    $finish;
  end
endmodule
