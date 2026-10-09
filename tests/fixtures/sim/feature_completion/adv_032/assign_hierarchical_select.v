// Nearest illegal form: a part-select of a hierarchical variable is not a
// procedural assign target (IEEE 1364-2001 9.3.1).
module leaf;
  reg [3:0] x;
endmodule

module tb;
  leaf u ();
  initial begin
    assign u.x[1:0] = 2'b01;
  end
endmodule
