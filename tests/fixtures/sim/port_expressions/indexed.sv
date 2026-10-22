// Indexed part-selects (`+:` and `-:` with constant bases) are constant
// selects, so they are legal port references.
module mo(.p(a[0+:2]), .q(a[3-:2]));
  output logic [3:0] a;
  initial a = 4'b1001;
endmodule

module mi(.p(n[2+:2]), .q(n[1-:2]));
  input [3:0] n;
endmodule

module tb;
  wire [1:0] w1, w2;
  mo c1(.p(w1), .q(w2));
  mi c2(.p(2'b10), .q(2'b01));
  initial #1 begin
    $display("w1=%b w2=%b n=%b", w1, w2, c2.n);
    $finish;
  end
endmodule
