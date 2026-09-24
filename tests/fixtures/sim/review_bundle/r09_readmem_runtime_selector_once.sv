module tb;
  logic [7:0] memory[0:1][0:1];
  int selector_calls;

  function automatic int select_row;
    selector_calls = selector_calls + 1;
    return 1;
  endfunction

  initial begin
    memory[0][0]=8'hee; memory[0][1]=8'hee;
    memory[1][0]=8'hee; memory[1][1]=8'hee;
    selector_calls = 0;
    $readmemh("words.hex", memory[select_row()]);
    if (memory[0][0] !== 8'hee || memory[0][1] !== 8'hee ||
        memory[1][0] !== 8'h11 || memory[1][1] !== 8'h22 ||
        selector_calls != 1)
      $fatal(1, "runtime row selector evaluation count");
    $display("PASS r09_readmem_runtime_selector_once calls=%0d", selector_calls);
    $finish;
  end
endmodule
