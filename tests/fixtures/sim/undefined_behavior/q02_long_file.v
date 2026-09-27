module tb;
  reg [7:0] mem [0:1];
  initial begin
    mem[0] = 8'hff; mem[1] = 8'hff;
    $readmemh("q02_long_file.mem", mem, 0, 1);
    $display("Q02.long_file values=%b,%b", mem[0], mem[1]);
  end
endmodule
