// Fixed storage remains fixed when a legal higher-dimension selector varies.
module tb;
  logic [7:0] memory[0:1][0:1];
  int row;
  initial begin
    memory[0][0]=8'hee; memory[0][1]=8'hee;
    memory[1][0]=8'hee; memory[1][1]=8'hee;
    row=1;
    $readmemh("words.hex",memory[row]);
    if (memory[0][0] !== 8'hee || memory[0][1] !== 8'hee ||
        memory[1][0] !== 8'h11 || memory[1][1] !== 8'h22)
      $fatal(1, "selected row load");
    $display("PASS r09_readmem_runtime_row");
    $finish;
  end
endmodule
