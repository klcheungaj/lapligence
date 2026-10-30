// Decision A20-D18: intermodule paths. vpi_handle_multi(vpiInterModPath,
// port1, port2) gives the intermodule path from output (or inout) port or
// port bit port1 to input (or inout) port or port bit port2 of the same
// size; other ports, the reverse order, one port twice, a size mismatch or
// another type are a vpiError and give NULL. Its delays (2 or 3: rise, fall,
// turn-off) are the module input port delays of its load port (decision
// A20-D17): a read gives them and a write sets them. vpi_iterate(vpiPort)
// gives its two ports, the source first; two handles for the same ports
// compare equal. It has no name, module or delay expression (decision
// A19-D26).
//
// IEEE 1800-2009 38.22 (SystemVerilog-1800-2009.txt L63418-L63420): "The
// VPI routine vpi_handle_multi() can be used to return a handle to an object
// of type vpiInterModPath associated with a list of output port and input
// port reference objects. The ports shall be of the same size and can be at
// different levels of the hierarchy."; 37.33 L59828-L59843 draws "inter mod
// path -> ports" and "-> delay vpi_get_delays() vpi_put_delays()";
// 38.10 L62275 and 38.32 L64014: "For intermodule path objects, the
// no_of_delays value shall be 2 or 3." Some simulators accept ports of
// different sizes, read other counts, treat a write as a no-op and refuse
// the port iteration; llg follows the clause text, which requires equal
// sizes, gives 2 or 3 delays, lists vpi_put_delays and draws the ports.
// Some simulators name an intermodule path after its load port; llg
// follows decision A19-D26 for properties the diagram does not draw.
//
// Build A20-D18_intermodule_paths.c as a VPI application (llg: static
// archive given to --dpi-lib; `task $d18()` declared as a system task).
`timescale 1ns/1ns
module src (input a, output o, output [1:0] ov);
  assign o = a;
  assign ov = {a, a};
endmodule

module snk (input i, input [1:0] v);
  always @(i) $display("%0d i=%b", $time, i);
endmodule

module tb;
  reg a = 1'b0;
  wire n;
  wire [1:0] nv;
  src s (.a(a), .o(n), .ov(nv));
  snk k (.i(n), .v(nv));
  initial begin
    #1 $d18();
    #9 a = 1'b1;
    #10 a = 1'b0;
    #10 $finish;
  end
endmodule
