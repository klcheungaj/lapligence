module tb;
  logic [7:0] memory[3:0];
  initial begin
    memory[0]=8'hee; memory[1]=8'hee; memory[2]=8'hee; memory[3]=8'hee;
    $readmemh("words.hex", memory[2:1]);
    if (memory[0] !== 8'hee || memory[1] !== 8'h11 ||
        memory[2] !== 8'h22 || memory[3] !== 8'hee)
      $fatal(1, "reversed declaration slice load");
    $display("PASS r09_readmem_reversed_decl");
    $finish;
  end
endmodule
