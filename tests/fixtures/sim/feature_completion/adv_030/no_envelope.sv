`pragma protect author="adv_030", author_info="protect keywords outside any envelope"
module pass_through(input logic a, output logic b);
`pragma protect comment="not an envelope"
  assign b = a;
endmodule

module tb;
  logic a, b;
  pass_through u (.a(a), .b(b));
  initial begin
    a = 1'b1;
    #1 $display("b=%b", b);
    $finish;
  end
endmodule
