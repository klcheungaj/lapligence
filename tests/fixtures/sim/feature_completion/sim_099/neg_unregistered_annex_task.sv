// $key (IEEE 1800-2009 D.5, informative) has no built-in lowering.
// It stays on the VPI path, where an application may provide it (36.3.2);
// with none registered the run stops at startup naming this call.
module tb;
  initial begin
    // The startup check stops the run before this display.
    $key;
    $display("unreachable");
  end
endmodule
