// Port expressions on generate-loop instances and on arrays of instances: an
// array instance gets its slice of a wider actual first (23.3.3.5), which the
// port expression then splits.
module m(.p({hi, lo[0]}));
  output hi;
  output [1:0] lo;
  assign hi = 1'b1;
  assign lo = 2'b10;
endmodule

module s(.p(r[1:0]));
  output logic [3:0] r;
  initial r = 4'b0110;
endmodule

module tb;
  wire [1:0] w [0:1];
  wire [3:0] x, y;
  genvar k;
  for (k = 0; k < 2; k++) begin : g
    m c(.p(w[k]));
  end
  m arr [1:0] (.p(x));
  s sa [1:0] (.p(y));
  initial #1 begin
    $display("w0=%b w1=%b x=%b y=%b", w[0], w[1], x, y);
    $finish;
  end
endmodule
