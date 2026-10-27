// ADV-015 A01: a checker bound into an RTL module observes the module's
// interface port, drives its own variables, samples a covergroup and
// instantiates a subchecker whose clock is inferred from the checker's
// default clocking (IEEE 1800-2009 17.2, 17.3, 17.4, 17.7.3, 23.11).
`timescale 1ns / 1ns
interface bus_if(input logic clk);
  logic req, ack;
endinterface

module dut(bus_if b);
  always @(posedge b.clk) b.ack <= b.req;
endmodule

checker ack_seen(logic ack, event ck = $inferred_clock);
  bit [3:0] acks = 4'd0;
  always @(ck) acks <= acks + (ack ? 4'd1 : 4'd0);
  final $display("%m acks=%0d", acks);
endchecker

checker handshake(logic req, logic ack, event clk);
  default clocking @clk; endclocking
  bit [3:0] pending = 4'd0;
  always @(clk) pending <= pending + (req ? 4'd1 : 4'd0) - (ack ? 4'd1 : 4'd0);
  covergroup cg @(clk);
    cp_req : coverpoint req;
    cp_pending : coverpoint pending { bins none = {0}; bins some = {[1:15]}; }
  endgroup
  cg cov = new();
  ack_seen u_seen(ack);
  a_ack: assert property (req |=> ack)
    else $display("%0t %m missing ack pending=%0d", $time, pending);
  final $display("%m pending=%0d cov=%0.2f", pending, cov.get_coverage());
endchecker

module tb;
  logic clk = 1'b0;
  always #5 clk = ~clk;
  bus_if bi(clk);
  dut d(bi);
  initial begin
    bi.req = 1'b0;
    bi.ack = 1'b0;
    @(negedge clk) bi.req = 1'b1;
    @(negedge clk) bi.req = 1'b0;
    @(negedge clk) begin
      bi.req = 1'b1;
      force bi.ack = 1'b0;
    end
    @(negedge clk) bi.req = 1'b0;
    @(negedge clk) release bi.ack;
    #20 $finish;
  end
endmodule

bind dut handshake hs(b.req, b.ack, posedge b.clk);
