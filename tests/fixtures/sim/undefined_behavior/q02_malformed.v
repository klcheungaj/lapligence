module tb;
  reg [7:0] mem [0:2];
  initial begin
    mem[0] = 8'hff; mem[1] = 8'hff; mem[2] = 8'hff;
    $readmemh("q02_malformed.mem", mem, 0, 2);
    $display("Q02.malformed values=%b,%b,%b", mem[0], mem[1], mem[2]);
  end
endmodule
