// R04 composition: resolve every address operand once and freeze the NBA mask.
module tb;
  logic [1:0][7:0] mem [0:1];
  int row, lane, index, count;
  initial begin
    mem[0] = 16'ha500;
    mem[1] = 16'h4321;
    row = 0; lane = 0; index = 6;
    mem[row++][lane++][index++ +: 4] <= 4'hf;
    if (row != 1 || lane != 1 || index != 7) $fatal(1, "NBA address not captured once");
    mem[0] = 16'h5a15;
    row = 1; lane = 1; index = 0;
    #1;
    if (mem[0] !== 16'h5ad5 || mem[1] !== 16'h4321)
      $fatal(1, "NBA clipped mask or issue address");
    mem[0] = 16'h5a08;
    row = 0; lane = 0; index = 2;
    mem[row++][lane++][index++ +: 3] += 3'b001;
    if (row != 1 || lane != 1 || index != 3 || mem[0] !== 16'h5a0c)
      $fatal(1, "compound address not captured once");
    mem[0] = 16'ha500;
    count = $sscanf("f", "%h", mem[0][0][6 +: 4]);
    if (count != 1 || mem[0] !== 16'ha5c0) $fatal(1, "selected file-input target");
    $display("packed selection capture passed");
    $finish(0);
  end
endmodule
