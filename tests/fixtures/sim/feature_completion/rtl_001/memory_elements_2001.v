// IEEE 1364-2001 section 3.10: memory declarations and element assignments.
module tb;
  reg [7:0] memory [2:1];
  initial begin
    memory[2] = 8'h5a;
    memory[1] = 8'ha5;
    $display("memory=%h:%h", memory[2], memory[1]);
    $finish(0);
  end
endmodule
