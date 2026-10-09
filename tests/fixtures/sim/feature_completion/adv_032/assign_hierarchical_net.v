// Nearest illegal form: a procedural assign target must be a variable
// (IEEE 1364-2001 9.3.1), so a hierarchical net is a language error.
module leaf;
  wire [3:0] n;
endmodule

module tb;
  reg [3:0] src;
  leaf u ();
  initial begin
    src = 4'h1;
    assign u.n = src;
  end
endmodule
