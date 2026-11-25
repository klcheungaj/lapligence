// SIM-011 boundary: a multidimensional instance property keeps packed cells in
// one fixed-size container; one of real cells (with or without an initializer)
// rejects at its declaration with its location. A static one is a variable.
class grid_c;
  real g[2][2] = '{'{1.0, 2.0}, '{3.0, 4.0}};
endclass
module tb;
  grid_c o = new;
  initial $display("%0.1f", o.g[1][0]);
endmodule
