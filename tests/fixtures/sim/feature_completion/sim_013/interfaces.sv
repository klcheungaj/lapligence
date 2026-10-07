// SIM-013 A01: event controls and a level wait on virtual-interface members.
// The waiter observes the member of the instance the variable names; a change
// in another instance does not wake it, and assigning the variable moves the
// observation to the new instance (SV 25.9, 9.4.2).
`timescale 1ns / 1ns
interface ifc;
  logic [3:0] sig;
  logic clk;
endinterface

module tb;
  ifc i0 ();
  ifc i1 ();
  virtual ifc v;
  string l_sig = "", l_clk = "";
  int t_wait = -1;

  initial begin
    i0.sig = 4'd1;
    i1.sig = 4'd2;
    i0.clk = 1'b0;
    i1.clk = 1'b0;
    v = i0;
  end

  initial #0 forever begin @(v.sig); l_sig = {l_sig, $sformatf(" %0t", $time)}; end
  initial #0 forever begin @(posedge v.clk); l_clk = {l_clk, $sformatf(" %0t", $time)}; end
  initial begin
    #0 wait (v.sig == 4'd9);
    t_wait = $time;
  end

  initial begin
    #1 i0.clk = 1'b1;
    #1 i0.sig = 4'd4;
    #1 v = i1;
    #1 i0.sig = 4'd5;
    #1 i0.clk = 1'b0;
    #1 i1.clk = 1'b1;
    #1 i1.sig = 4'd6;
    #1 i0.sig = 4'd9;
    #1 i1.sig = 4'd9;
    #1;
    $display("v.sig:%s", l_sig);
    $display("posedge v.clk:%s", l_clk);
    $display("wait: %0d", t_wait);
    $finish(0);
  end
endmodule
