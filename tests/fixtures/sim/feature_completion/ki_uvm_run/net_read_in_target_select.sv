`timescale 1ns/1ns
// A register file indexed by interface nets: the nets are read in the
// select of the left-hand side, and another driver drives them.
interface bus_if(input bit clk);
  wire [7:0] addr;
  wire [7:0] wdata;
  wire write;
  clocking mck @(posedge clk);
    output addr, wdata, write;
  endclocking
endinterface

module slave(bus_if bus);
  logic [15:0] regs[4];
  logic [7:0] flat[16];
  always @(posedge bus.clk)
    if (bus.write) begin
      regs[bus.addr[1:0]][15:8] <= bus.wdata;
      regs[bus.addr[1:0]][bus.addr[2] +: 4] <= bus.wdata[3:0];
      flat[bus.addr[3:0]] <= bus.wdata;
    end
  final $display("regs %h %h %h %h flat[5]=%h", regs[0], regs[1], regs[2], regs[3], flat[5]);
endmodule

module tb;
  bit clk;
  bus_if b(clk);
  slave s(b);
  always #5 clk = !clk;
  initial begin
    @(b.mck);
    b.mck.addr <= 8'h05;
    b.mck.wdata <= 8'ha7;
    b.mck.write <= 1;
    @(b.mck);
    b.mck.addr <= 8'h02;
    b.mck.wdata <= 8'h3c;
    @(b.mck);
    b.mck.write <= 0;
    @(b.mck);
    @(b.mck);
    $finish;
  end
endmodule
