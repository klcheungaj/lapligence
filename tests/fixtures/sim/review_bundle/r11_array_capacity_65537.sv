// Former 65,536-cell ceiling; IEEE 1800-2009 §7.4.2 requires at least 2^24.
module tb;
  reg memory[0:65536];
  initial begin
    memory[65536]=1'b1;
    if (memory[65536] !== 1'b1) $finish(0);
    $display("PASS r11_array_capacity_65537");
    $finish(0);
  end
endmodule
