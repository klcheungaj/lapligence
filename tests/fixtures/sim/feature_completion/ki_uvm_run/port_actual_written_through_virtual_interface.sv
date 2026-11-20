`timescale 1ns/1ns
// The bus model's arbiter drives the read line through an output port and
// rewrites it with an unchanged value at every clock edge, while a master's
// driver writes the same interface variable through a virtual interface.
module arbiter(input wire clk, output reg read, output reg write, output reg grant);
  int n = 0;
  always @(posedge clk) begin
    n++;
    read <= (n == 6) ? 1'b0 : 1'bz;
    write <= 1'b0;
    grant <= 1'b0;
  end
endmodule

interface bus_if;
  logic clk = 0;
  logic read;
  logic write;
  logic grant;
  clocking cb @(posedge clk);
    output write;
  endclocking
endinterface

class master;
  virtual bus_if vif;
  task run();
    @(posedge vif.clk);
    @(posedge vif.clk); vif.read <= 1'b1;
    @(posedge vif.clk); vif.read <= 1'b0; vif.cb.write <= 1'b1;
    @(posedge vif.clk); vif.read <= 1'bz;
    @(posedge vif.clk); vif.read = 1'b1;
    @(posedge vif.clk); vif.read <= 1'b1; vif.cb.write <= 1'b0;
    @(posedge vif.clk);
  endtask
endclass

module tb;
  bus_if bus();
  arbiter arb(bus.clk, bus.read, bus.write, bus.grant);
  always #5 bus.clk = ~bus.clk;
  always @(negedge bus.clk)
    if ($time > 0)
      $display("%0t read=%b write=%b grant=%b arb.read=%b arb.write=%b", $time, bus.read,
               bus.write, bus.grant, arb.read, arb.write);
  initial begin
    static master m = new;
    m.vif = bus;
    fork m.run(); join_none
    #78 $finish;
  end
endmodule
