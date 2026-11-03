// Decision A21-D6: a model that a VPI application reaches merges the same
// net ports as one without VPI (decisions A21-D2, A21-D4). An input or an
// output port between two nets is one simulated net: vpi_put_value (no
// delay) or a VPI force through either name changes both names, a release
// through either name releases both, and a cbValueChange on either name
// fires for every change of the merged net. A port whose source is a
// variable stays a continuous assignment: a put or force on its net sink
// leaves the variable alone. A merged port has no side for module input
// port delays to delay: vpi_get_delays and vpi_put_delays on it fail with
// an error and change nothing (llg generates a model with separate ports
// for such delays with --vpi-port-delays, decisions A20-D17, A20-D18).
//
// IEEE 1800-2009 23.3.3.7 (SystemVerilog-1800-2009.txt L41596-41598):
//   "It is permissible to merge the dominating and dominated nets into a
//   single net, whose type shall be that of the dominating net. The resulting
//   net is called the simulated net, and the dominated net is called a
//   collapsed net."
// IEEE 1800-2009 38.34 (L64213-64215):
//   "When vpi_put_value() is called for an object of type vpiNet or vpiNetBit,
//   and with modes of vpiInertialDelay, vpiTransportDelay,
//   vpiPureTransportDelay, or vpiNoDelay, the value supplied overrides the
//   resolved value of the net. This value shall remain in effect until one of
//   the drivers of the net changes value."
// IEEE 1800-2009 38.34 (L64186-64190): vpiForceFlag "The object shall be
//   forced to the passed value with no delay (same as the SystemVerilog
//   procedural force)"; vpiReleaseFlag "The object shall be released from a
//   forced value".
// IEEE 1800-2009 37.14 (L58166-58168): ports draw "-> delay (mipd)
//   vpi_get_delays() vpi_put_delays()"; 37.3.4 (L57419-57420): "Additional
//   delays may exist, such as module input port delays or inter-module path
//   delays, that do not appear within the SystemVerilog source code."
//
// The text permits the merge and does not say whether a merged port keeps
// module input port delays. llg follows the industry-standard behaviour.
//
// Build A21-D6_vpi_net_ports_merge.c as a VPI application (llg: static
// archive given to --dpi-lib; `task $d6(input integer stage)` declared as a
// system task).
`timescale 1ns/1ns
module child(input w, input c, output o, output reg q);
  reg ro = 1'b0;
  assign o = ro;
  always @(c) q = c;
endmodule

module tb;
  reg drv = 1'b0;
  reg clk = 1'b0;
  wire wn, on, qn;
  assign wn = drv;
  child u(.w(wn), .c(clk), .o(on), .q(qn));
  task show(input [8*10:1] tag);
    $display("%0s: wn=%b u.w=%b clk=%b u.c=%b u.o=%b on=%b u.q=%b qn=%b", tag,
             wn, u.w, clk, u.c, u.o, on, u.q, qn);
  endtask
  initial begin
    #1 $d6(0);
    #1 drv = 1'b1;
    clk = 1'b1;
    u.ro = 1'b1;
    #1 show("drivers 1");
    $d6(1);
    show("put sinks");
    #1 drv = 1'b0;
    clk = 1'b0;
    u.ro = 1'b0;
    #1 show("drivers 0");
    $d6(2);
    show("put srcs");
    #1 $d6(3);
    #1 show("forced");
    drv = 1'b1;
    clk = 1'b1;
    u.ro = 1'b1;
    #1 show("drv forced");
    $d6(4);
    #1 show("released");
    $d6(5);
    #1 drv = 1'b0;
    #1 $display("u.w=%b at %0t", u.w, $time);
    #3 $display("u.w=%b at %0t", u.w, $time);
    $d6(6);
    $finish;
  end
endmodule
