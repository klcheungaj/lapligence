// An actual with a side effect cannot be built when the report prints: the
// Postponed region writes nothing (SV 4.4.2.9), and building it at the call
// would read the wrong values (strobe_built_actuals.sv builds the others).
module tb;
  int q[$];
  function automatic int sumd(int x[]);
    int t = 0;
    foreach (x[i]) t += x[i];
    return t;
  endfunction
  initial begin
    q.push_back(1);
    $strobe("%0d", sumd({q.pop_back(), 2}));
  end
endmodule
