`timescale 1ns/1ns
interface ifc(input logic clk);
  logic [7:0] d, mem[4];
  logic s;
  real r;
endinterface

module tb;
  logic clk = 0; always #5 clk = ~clk;
  ifc i0(clk), i1(clk);
  virtual ifc vif;
  virtual ifc va[2];
  event e;
  class drv;
    virtual ifc v;
    task go();
      v.d <= #3 8'h44;
      v.mem[2] <= #4 8'h55;
      v.d[3:0] <= @e 4'hc;
    endtask
  endclass
  drv dr = new;
  int k = 1;
  always @(i0.d) $display("%0t i0.d=%h", $time, i0.d);
  always @(i0.mem[2], i0.mem[1]) #0 $display("%0t mem %h %h", $time, i0.mem[1], i0.mem[2]);
  always @(i0.s) #0 #0 $display("%0t s=%b", $time, i0.s);
  always @(i0.r) #0 #0 #0 $display("%0t r=%f", $time, i0.r);
  always @(i1.d) #0 #0 #0 #0 $display("%0t i1.d=%h", $time, i1.d);
  function void bind_to(virtual ifc v, int index);
    vif = v;
    k = index;
  endfunction
  initial begin
    va[0] = i0; va[1] = i1; dr.v = i0;
  end
  initial begin
    #0 bind_to(i0, 1);
    vif.d <= #2 8'h11;
    vif.mem[k] <= #2 8'h22;
    vif.s <= #1 1;
    vif.r <= #2 2.5;
    vif.d[7:4] <= #5 4'h9;
    va[k].d <= #1 8'h77;
    bind_to(i1, 0);
    dr.go();
    #8 -> e;
    #1 bind_to(i0, 0); vif.d <= repeat (2) @(posedge clk) 8'haa;
    bind_to(i1, 0);
    #40 {vif.d, vif.s} <= #1 9'h1ff;
    #5 $display("%h %h %b", i1.mem[1], i1.d, i1.s);
    $finish(0);
  end
endmodule
