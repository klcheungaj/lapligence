// SIM-011 boundary: a multidimensional class property keeps packed cells in
// one fixed-size container; one of real cells (static or not, with or without
// an initializer) rejects at its declaration with its location.
class grid_c;
  static real g[2][2] = '{'{1.0, 2.0}, '{3.0, 4.0}};
endclass
module tb;
  initial $display("%0.1f", grid_c::g[1][0]);
endmodule
