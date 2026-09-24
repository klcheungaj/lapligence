// IEEE 1800-2009 21.4 explicitly permits the lowest specified slice.
module tb;
  logic [7:0] memory[0:3];
  initial begin
    memory[0]=8'hee; memory[1]=8'hee; memory[2]=8'hee; memory[3]=8'hee;
    $readmemh("words.hex",memory[1:2]);
    if (memory[0] !== 8'hee || memory[1] !== 8'h11 ||
        memory[2] !== 8'h22 || memory[3] !== 8'hee)
      $fatal(1, "selected slice load");
    $display("PASS r09_readmem_slice");
    $finish;
  end
endmodule
