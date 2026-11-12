`timescale 1ns/1ns
// Static initializers of packages and of the design: a library numbers each
// object it creates, and a testbench module creates objects in its own
// declaration initializers.
package counter_pkg;
  class obj;
    static int count;
    int id;
    function new(string tag);
      id = count++;
      $display("%0d %s", id, tag);
    endfunction
  endclass
endpackage

package lib_pkg;
  import counter_pkg::*;
  obj top = new("lib_pkg.top");
  class registry #(type T = int);
    static obj me = new("registry.me");
  endclass
  class singleton;
    static obj inst = new("singleton.inst");
  endclass
  obj after = new("lib_pkg.after");
endpackage

package user_pkg;
  import counter_pkg::*;
  obj cfg = new("user_pkg.cfg");
endpackage

module tb;
  import counter_pkg::*;
  obj unit = new("tb.unit");
  obj data = new("tb.data");
  typedef lib_pkg::registry #(byte) byte_registry;
  initial begin
    $display("unit=%0d data=%0d top=%0d after=%0d cfg=%0d me=%0d inst=%0d", unit.id, data.id,
             lib_pkg::top.id, lib_pkg::after.id, user_pkg::cfg.id, byte_registry::me.id,
             lib_pkg::singleton::inst.id);
    $finish;
  end
endmodule
