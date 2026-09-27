module tb;
  reg [7:0] mem [0:3];
  initial begin
    mem[0] = 8'hff; mem[1] = 8'hff; mem[2] = 8'hff; mem[3] = 8'hff;
    $readmemh("q02_short_file.mem", mem, 0, 3);
    $display("Q02.short_file values=%b,%b,%b,%b", mem[0], mem[1], mem[2], mem[3]);
  end
endmodule
