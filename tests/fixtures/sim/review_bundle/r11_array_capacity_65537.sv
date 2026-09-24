// One cell above the project limit, still far below the 2^24 LRM minimum.
module tb;
  reg memory[0:65536];
  initial begin
    memory[65536]=1'b1;
    if (memory[65536] !== 1'b1) $finish;
    $display("PASS r11_array_capacity_65537");
    $finish;
  end
endmodule
