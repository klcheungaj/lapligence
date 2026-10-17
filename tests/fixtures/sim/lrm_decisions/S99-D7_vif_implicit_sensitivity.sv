// Decision S99-D7: the implicit sensitivity of always_comb, always_latch and
// @* includes members read through a virtual-interface handle. The procedure
// is sensitive to the member of the interface instance the handle binds
// (packed members and elements of unpacked array members), and to the handle
// variable itself: rebinding the handle re-runs the procedure, which then
// follows the new instance. Changes of an instance the handle does not bind
// do not re-run it.
//
// IEEE 1800-2009 9.2.2.2.1 (SystemVerilog-1800-2009.txt L11184-11186): "The
// implicit sensitivity list of an always_comb includes the expansions of the
// longest static prefix of each variable or select expression that is read
// within the block or within any function called within the block". 9.4.2.2
// L11899: "All net and variable identifiers that appear in the statement will
// be automatically added to the event expression". 25.9 L44930-44932: "all
// the components of the underlying interface instance are directly available
// to the virtual interface via the dot notation. These components can only be
// used in procedural statements; they cannot be used in continuous
// assignments or sensitivity lists." The text does not say what the implicit
// list of a procedure that reads them holds (25.9's own example waits on
// `@(posedge bus.grant)`, so the restriction is read as one on explicit
// lists). llg reading: the procedure follows the bound instance.
`timescale 1ns/1ns
interface bus_if;
  logic [7:0] a;
  logic [7:0] mem [0:3];
endinterface

module tb;
  bus_if x();
  bus_if y();
  virtual bus_if v = x;
  int runs = 0;
  logic [7:0] c_a, c_el, s_a;

  always_comb c_a = v.a;
  always_comb c_el = v.mem[2];
  always @* begin
    s_a = v.a;
    runs++;
  end

  initial begin
    #1;
    x.a = 8'h15;
    x.mem[2] = 8'h12;
    y.a = 8'h26;
    y.mem[2] = 8'h22;
    #1 $display("%0t comb a=%h el=%h star a=%h runs=%0d", $time, c_a, c_el, s_a, runs);
    y.a = 8'h48;
    y.mem[2] = 8'h42;
    #1 $display("%0t comb a=%h el=%h star a=%h runs=%0d", $time, c_a, c_el, s_a, runs);
    v = y;
    #1 $display("%0t comb a=%h el=%h star a=%h runs=%0d", $time, c_a, c_el, s_a, runs);
    x.a = 8'h59;
    #1 $display("%0t comb a=%h el=%h star a=%h runs=%0d", $time, c_a, c_el, s_a, runs);
    y.a = 8'h6a;
    y.mem[2] = 8'h62;
    #1 $display("%0t comb a=%h el=%h star a=%h runs=%0d", $time, c_a, c_el, s_a, runs);
    $finish;
  end
endmodule
