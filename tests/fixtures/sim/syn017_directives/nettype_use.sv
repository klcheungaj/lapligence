module tb;
  wire source;
  assign implicit_wire = source;
  initial begin
    #0;
    $display("implicit=%b", implicit_wire);
    $finish;
  end
endmodule
