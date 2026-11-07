module tb;
  int a3[string][string][int];
  int aq[string][int][$];
  int pa[string][int];
  int qa[$][int];
  initial begin
    pa = '{"x": '{1: 2, 3: 4}, "y": '{5: 6}};
    $display("%0d %0d %0d", pa.num(), pa["x"].num(), pa["y"][5]);
    qa = '{'{7: 1}, '{8: 2}};
    $display("%0d %0d", qa.size(), qa[1][8]);
    a3["a"]["b"][1] = 5;
    $display("%0d %0d", a3["a"]["b"][1], a3["a"].num());
    aq["k"][2].push_back(4);
    aq["k"][2].push_back(5);
    $display("%0d %0d", aq["k"][2].size(), aq["k"][2][1]);
    foreach (a3[i, j, k]) $display("%s %s %0d", i, j, k);
    $finish;
  end
endmodule
