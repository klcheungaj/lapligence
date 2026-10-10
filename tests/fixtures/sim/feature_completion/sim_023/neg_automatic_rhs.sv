// SV 13.3.2: a force statement cannot reference an automatic variable.
module tb;
  logic [3:0] v;
  initial begin
    automatic logic [3:0] k = 4'h1;
    force v = k;
  end
endmodule
