// A deferred assertion action passing an associative array by value: a
// located limit (docs/known_issues.md, deferred assertion actions with
// container, event or virtual-interface arguments).
module tb;
  int aa[int];
  function automatic void report(int m[int]);
    $display("size %0d", m.num());
  endfunction
  initial begin
    aa[1] = 1;
    assert #0 (aa.num() == 0) else report(aa);
    #1 $finish(0);
  end
endmodule
