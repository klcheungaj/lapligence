`timescale 1ns/1ns
// Bit-vector functions in concurrent assertions read the sampled values of
// their arguments: a bus interface checks its address and grant lines from
// a procedure, as a verification component's interface does.
interface bus_if(input logic clk);
  bit has_checks = 1;
  logic [15:0] grant;
  logic [15:0] addr;
  logic [3:0] req = 0;

  always @(negedge clk) begin
    addr_known: assert property (disable iff (!has_checks)
                                 ($onehot(grant) |-> !$isunknown(addr)))
      else $display("%0t addr_known failed", $time);
    grant_one: assert property (disable iff (!has_checks)
                                ($onehot0(grant)))
      else $display("%0t grant_one failed", $time);
  end
endinterface

module tb;
  logic clk = 1;
  bus_if b(clk);

  // Module-level forms with an explicit clock.
  req_count: assert property (@(posedge clk) $countones(b.req) <= 2)
    else $display("%0t req_count failed", $time);
  req_known: assert property (@(posedge clk) $isunknown(b.req) |=> b.req == 4'b0101)
    else $display("%0t req_known failed", $time);

  always #5 clk = !clk;

  initial begin
    b.grant = 0; b.addr = 'x;
    @(posedge clk);            // 10
    b.grant = 16'h0002;        // sampled at 15: one hot, addr unknown
    @(posedge clk);            // 20
    b.addr = 16'h0010;
    b.grant = 16'h0003;        // sampled at 25: two grants
    @(posedge clk);            // 30
    b.grant = 0;
    b.req = 4'b0111;           // written at the edge: the 30 check sees 0
    @(posedge clk);            // 40: three requests sampled
    b.req = 4'bx001;
    @(posedge clk);            // 50: unknown sampled
    b.req = 0;
    @(posedge clk);            // 60: next cycle is not 0101
    b.has_checks = 0;
    b.grant = 16'h0003;
    @(posedge clk);            // 70
    @(posedge clk);            // 80
    $display("done");
    $finish;
  end
endmodule
