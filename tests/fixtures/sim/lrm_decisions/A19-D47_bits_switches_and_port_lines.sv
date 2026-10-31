// Decision A19-D47: bits, pull primitives and port lines. A bit, element,
// select or member of a declared net or variable reports its declaration's
// vpiLineNo. vpi_iterate(vpiBit, ...) on a scalar net, variable or port (one
// bit without a packed dimension) returns NULL; a vector iterates its bits.
// A pullup or pulldown is a vpiSwitch. A non-ANSI port reports the line of
// its module header entry; its io declaration and the net it declares keep
// their own line.
//
// IEEE 1800-2009 37.3.3 (SystemVerilog-1800-2009.txt L57389-57396): "These
// properties are applicable to every object that corresponds to some object
// within the source code." 37.16 detail 25 (L58578): "A logic net without a
// packed dimension defined is a scalar". 37.31 (L59736-59780) lists gate,
// switch and udp primitives without assigning pullup and pulldown to one of
// them. 23.2.2.1 (L40406) declares a non-ANSI port in the header and its
// direction in an io declaration. The text is silent on these answers; they
// follow industry-standard behaviour.
//
// Build A19-D47_bits_switches_and_port_lines.c as a VPI application (llg:
// static archive given to --dpi-lib; `task $d47()` declared as a system
// task).
module leaf(p,
            z);
  input p;
  input [1:0] z;
endmodule
module tb;
  wire a = 1'b1;
  wire [3:0] nv;
  reg [3:0] rv;
  reg rs;
  wire [3:0] na [0:1];
  wire d, e, f, g, h, c;
  assign nv = 4'h3;
  and g1 (c, a, a);
  pullup (d);
  bufif1 bz (h, a, a);
  pulldown (strong0) pd (e);
  tran t2 (g, h);
  pullup pu2 (f);
  leaf k (a, nv[1:0]);
  initial begin
    #1 $d47;
  end
endmodule
