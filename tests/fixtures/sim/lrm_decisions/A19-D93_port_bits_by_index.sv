// Decision A19-D93: port bits by index. IEEE 1800-2009 37.14 draws "access
// by index vpi_handle_by_index() vpi_handle_by_multi_index()" on ports
// (SystemVerilog-1800-2009.txt L58161-58163), and 38.19 (L63300-63305):
// "The reference object shall be an object that has the access by index
// property. ... If the selection represented by the index number does not
// lead to the construction of a legal SystemVerilog index select
// expression, the routine shall return a null handle." 38.20 (L63347-63348):
// "The order of the indices provided shall follow the array dimension
// declaration from the leftmost range to the rightmost range of the
// reference handle".
// Decision: vpi_handle_by_index(port, i) is the port bit that index i
// selects in the packed range of the declaration the port connects (the
// same object the vpiBit iteration gives: index 3 of [3:0] is the first
// bit, index 0 of [0:3] too); a port with a module-side select or
// concatenation has the range [size-1:0]; a two-dimensional packed port
// takes vpi_handle_by_multi_index with both indices. An index outside the
// range gives NULL (with an error for a packed array, as on its
// declaration); a one-bit port has no bits (A19-D47) and gives NULL; an
// interface port, a port bit and a wrong index count give NULL with an
// error. The bits connect, on each side, the bit of the object or of the
// concatenation operand (a select included) that holds them.
// Some simulators refuse vpi_handle_by_index and vpi_handle_by_multi_index
// on every port; llg follows the diagram.
//
// Build A19-D93_port_bits_by_index.c as a VPI application (llg: static
// archive given to --dpi-lib; `task $d93()` declared as a system task).
interface ifc;
  logic s;
  modport mp (input s);
endinterface
module ansi (input logic [3:0] a, input logic [0:3] b, output logic [7:4] c,
             input logic s, input logic [-1:-3] n, input logic [1:0][2:0] pk,
             ifc.mp m);
  assign c = a ^ b ^ {3'b000, s} ^ {1'b0, n} ^ pk[1][3:0];
  wire unused = m.s;
endmodule
module legacy (x, .z({p, q}), .r(t[4:2]));
  input [5:2] x;
  input [1:0] p;
  input q;
  input [4:0] t;
  wire unused = ^{x, p, q, t};
endmodule
module tb;
  logic [3:0] ta = 4'b1010, tb_ = 4'b0110;
  logic [7:0] wide = 8'hc5;
  logic ts = 1'b1;
  logic [2:0] tn = 3'b011;
  logic [5:0] tpk = 6'b101100;
  wire [3:0] tc;
  logic [2:0] tr = 3'b100;
  ifc i0 ();
  ansi u (.a(ta), .b(tb_), .c(tc), .s(ts), .n(tn), .pk(tpk), .m(i0.mp));
  legacy l (.x(wide[6:3]), .z({ts, tn[1:0]}), .r(tr));
  initial begin
    #1;
    $d93;
    $finish;
  end
endmodule
