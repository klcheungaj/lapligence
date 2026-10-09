// Nearest illegal form: force/release admits a whole variable but not a
// select of one (IEEE 1364-2001 9.3.2).
module leaf;
  reg [3:0] x;
endmodule

module tb;
  leaf u ();
  initial begin
    force u.x[1] = 1'b1;
  end
endmodule
