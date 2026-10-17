// IEEE 1800-2009 36.3.2: sreadmem_override.c registers $sreadmemh, which this
// design does not call, so no built-in is overridden: $sreadmemb runs without
// any VPI registration of its own.
module tb;
  logic [7:0] mem [0:1];
  initial begin
    $sreadmemb(mem, 0, 1, "00010001 0010_0010");
    $display("%h %h", mem[0], mem[1]);
    $finish;
  end
endmodule
