// A library agent's driver and monitors reach the interfaces nested in the
// agent's interface through its virtual interface: the driver assigns one to
// its own handle, and the agent passes the others to a parameterized
// configuration class.
`timescale 1ns/1ns
interface tx_if(input bit clk, output reg tx);
endinterface
interface rx_if(input bit clk, input reg rx);
endinterface
interface agent_if(input bit clk, output reg tx, input wire rx);
  tx_if txi(clk, tx);
  rx_if rxi(clk, rx);
  rx_if tx_mon(clk, tx);
endinterface
class config_db #(type T = int);
  static T entries[string];
  static function void set(string name, T value);
    entries[name] = value;
  endfunction
  static function bit get(string name, inout T value);
    if (!entries.exists(name)) return 0;
    value = entries[name];
    return 1;
  endfunction
endclass
class agent;
  virtual agent_if vif;
  function void build();
    config_db#(virtual rx_if)::set("tx_mon", vif.tx_mon);
    config_db#(virtual rx_if)::set("rx_mon", vif.rxi);
  endfunction
endclass
class driver;
  agent parent;
  virtual tx_if vif;
  task run();
    vif = parent.vif.txi;
    @(posedge vif.clk) vif.tx <= 1;
    @(posedge vif.clk) $display("%0t tx=%b", $time, vif.tx);
  endtask
endclass
class monitor;
  string name;
  virtual rx_if vif;
  function new(string name);
    this.name = name;
    void'(config_db#(virtual rx_if)::get(name, vif));
  endfunction
  task run();
    @(negedge vif.clk) $display("%0t %s rx=%b", $time, name, vif.rx);
  endtask
endclass
module tb;
  bit clk;
  wire tx;
  reg rx = 0;
  agent_if a(clk, tx, rx);
  always #5 clk = !clk;
  initial begin
    agent ag = new;
    driver d = new;
    monitor m;
    ag.vif = a;
    ag.build();
    d.parent = ag;
    d.run();
    m = new("tx_mon");
    m.run();
    m = new("rx_mon");
    m.run();
    $finish;
  end
endmodule
