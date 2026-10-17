// IEEE 1800-2009 36.3.2 with decision S99-D6: sreadmem_override.c registers
// $sreadmemh, which this design calls; llg stops at startup instead of
// running its built-in or the application's task.
module tb;
  logic [7:0] mem [0:1];
  initial begin
    $sreadmemh(mem, 0, 1, "11 22");
    $display("unreachable %h", mem[0]);
  end
endmodule
