module tb;
  logic [7:0] memory[0:3];
  initial begin
    memory[0]=8'hee; memory[1]=8'hee; memory[2]=8'hee; memory[3]=8'hee;
    $readmemh("words.hex", memory[1:2], 0, 1);
    if (memory[0] !== 8'hee || memory[1] !== 8'hee ||
        memory[2] !== 8'hee || memory[3] !== 8'hee)
      $fatal(1, "slice address bounds must reject unselected addresses");
    $display("PASS r09_readmem_slice_address_bounds");
    $finish;
  end
endmodule
