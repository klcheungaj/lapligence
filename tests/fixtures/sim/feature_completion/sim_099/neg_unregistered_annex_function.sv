// $reset_count (IEEE 1800-2009 D.8, informative) has no built-in lowering.
// It stays on the VPI path, where an application may provide it (36.3.2);
// with none registered the run stops at startup naming this call.
module tb;
  integer resets;
  initial begin
    resets = $reset_count;
    $display("%0d", resets);
  end
endmodule
