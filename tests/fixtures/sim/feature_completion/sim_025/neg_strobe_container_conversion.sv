// A queue actual of a dynamic-array formal is a conversion the report would have
// to run at the end of the slot; no evaluator form exists yet.
module tb;
  int q[$];
  function automatic int sumd(int x[]);
    int t = 0;
    foreach (x[i]) t += x[i];
    return t;
  endfunction
  initial begin
    q.push_back(1);
    $strobe("%0d", sumd(q));
  end
endmodule
