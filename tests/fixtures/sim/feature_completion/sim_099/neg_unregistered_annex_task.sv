// $sreadmemh (IEEE 1800-2009 D.14, informative) has no built-in lowering.
// It stays on the VPI path, where an application may provide it (36.3.2);
// with none registered the run stops at startup naming this call.
module tb;
  logic [7:0] mem[0:1];
  initial begin
    $sreadmemh(mem, 0, 1, "1f 2e");
    $display("%h", mem[0]);
  end
endmodule
