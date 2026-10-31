// Decision A19-D51: the definition properties of a package. A package under
// `celldefine and `unconnected_drive is neither a cell nor pulled; its
// `default_nettype and default lifetime are its own.
`timescale 1ns/1ps
`default_nettype tri
`unconnected_drive pull1
`celldefine
package pk;
  int x;
endpackage
`endcelldefine
`nounconnected_drive
`default_nettype wire
package automatic pa;
  int y;
endpackage
module tb;
  initial #1 $d51;
endmodule
