// A nested interface instance selected through a virtual interface (SV 25.9):
// as the source of a virtual-interface assignment, as an actual argument and
// as an operand of a handle comparison, as a UVM driver selects its transmit
// interface (`vif = agent.vif.tx`).
`timescale 1ns/1ns
interface tx_if(input bit clk, output reg tx);
endinterface
interface rx_if(input bit clk, input reg rx);
endinterface
interface top_if(input bit clk, output reg tx, input wire rx);
  tx_if txi(clk, tx);
  rx_if rxi(clk, rx);
  rx_if tx_mon(clk, tx);
endinterface
class monitor;
  virtual rx_if rxv;
  virtual rx_if txv;
  function new(virtual rx_if r, virtual rx_if t);
    rxv = r;
    txv = t;
  endfunction
  task run();
    repeat (3) begin
      @(posedge rxv.clk);
      $display("%0t rx=%b tx_mon=%b", $time, rxv.rx, txv.rx);
    end
  endtask
endclass
class driver;
  virtual top_if top;
  virtual tx_if txv;
  task run();
    txv = top.txi;
    @(negedge txv.clk) txv.tx = 1;
    @(negedge txv.clk) txv.tx = 0;
    @(negedge txv.clk) txv.tx = 1;
  endtask
endclass
module tb;
  bit clk;
  wire tx;
  reg rx;
  top_if t(clk, tx, rx);
  always #5 clk = !clk;
  initial begin
    driver d = new;
    monitor m;
    virtual tx_if other;
    rx = 0;
    d.top = t;
    m = new(d.top.rxi, d.top.tx_mon);
    fork
      d.run();
      m.run();
      #12 rx = 1;
    join
    other = d.top.txi;
    $display("same=%0d rx_vs_mon=%0d tx=%b", d.txv == other, m.rxv == m.txv, tx);
    $finish;
  end
endmodule
