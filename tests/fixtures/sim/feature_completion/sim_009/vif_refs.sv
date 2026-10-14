// SIM-009-A03: a waited virtual-interface `ref` formal is a typed handle
// parameter: the waits follow the interface the variable names when they
// arm, also after the caller rebinds it, and the task recurses (SV 9.4.2,
// 13.5.2, 25.9).
interface ifc;
  logic clk = 0;
  int data = 0;
endinterface

module tb;
  ifc i0();
  ifc i1();
  virtual ifc vi;

  task automatic tick(ref virtual ifc v, input int n);
    @(posedge v.clk);
    $display("tick %0d at %0d data %0d", n, $time, v.data);
    if (n > 0) tick(v, n - 1);
  endtask

  task automatic settle(ref virtual ifc v, input int n);
    wait (v.data == n);
    $display("settle %0d at %0d", n, $time);
  endtask

  initial begin
    vi = i0;
    fork
      tick(vi, 2);
      settle(vi, 7);
    join_none
    #1 i0.data = 1;
    i0.clk = 1;
    #1 i0.clk = 0;
    #1 vi = i1;
    #1 i0.clk = 1;
    #1 i1.data = 5;
    i1.clk = 1;
    #1 i1.clk = 0;
    i0.data = 7;
    #1 i1.data = 7;
    #1 i1.clk = 1;
    #1 $finish;
  end
endmodule
