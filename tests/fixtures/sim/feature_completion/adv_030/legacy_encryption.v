module plain_legacy(a, b);
  input a;
  output b;
`protect
  assign b = a;
`endprotect
endmodule

module tb;
  reg a;
  wire b;
  plain_legacy u (.a(a), .b(b));
  initial begin
    a = 1'b1;
    #1 $display("b=%b", b);
    $finish;
  end
endmodule
