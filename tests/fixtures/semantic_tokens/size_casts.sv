// Size casts (ST-52): the names of a cast width are references in both
// serving paths, looked up in the scope of the cast.
package sc_pkg;
  localparam int SC_P = 4;
endpackage

module size_casts import sc_pkg::*; (
  input  logic [7:0]  a,
  output logic [15:0] y,
  output logic [15:0] z,
  output logic [15:0] v,
  output logic [15:0] u
);
  localparam int SC_W = 16;
  localparam int SC_K = 8;
  typedef logic [15:0] sc_t;
  assign y = (SC_W - SC_K)'(a);
  assign z = SC_W'(a) + sc_t'(a) + sc_pkg::SC_P'(a);
  always_comb begin : blk
    localparam int SC_W = 12;
    v = SC_W'(a);
  end
  initial begin
    u = (SC_P + SC_K)'(a);
  end
endmodule
