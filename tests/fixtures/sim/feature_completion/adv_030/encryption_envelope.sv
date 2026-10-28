module plain_enc(input logic a, output logic b);
`pragma protect data_method="aes128-cbc", begin
  assign b = a;
`pragma protect end
endmodule

module tb;
  logic a, b;
  plain_enc u (.a(a), .b(b));
  initial begin
    a = 1'b1;
    #1 $display("b=%b", b);
    $finish;
  end
endmodule
