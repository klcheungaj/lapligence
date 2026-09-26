// The else arm is checked only when it is the selected branch.
`ifdef TAKE_IF
module tb;
  initial begin
    $display("if");
    $finish;
  end
endmodule
`else
`pragma diagnostic push
module tb;
  initial begin
    $display("else");
    $finish;
  end
endmodule
`pragma diagnostic pop
`endif
