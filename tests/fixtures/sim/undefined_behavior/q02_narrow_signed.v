module tb;
  reg signed [3:0] mem [0:4];
  initial begin
    $readmemh("q02_hex.mem", mem, 0, 4);
    $display("Q02.narrow_signed x=%b z=%b 1x=%b x1=%b zX=%b", mem[0], mem[1], mem[2], mem[3], mem[4]);
  end
endmodule
