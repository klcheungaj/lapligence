module tb;
  reg signed [0:0] mem [0:4];
  initial begin
    $readmemb("q02_binary.mem", mem, 0, 4);
    $display("Q02.binary_narrow x=%b z=%b 1x=%b x1=%b zX=%b", mem[0], mem[1], mem[2], mem[3], mem[4]);
  end
endmodule
