module tb;
  int aa[string][int];
  int qa[$][int];
  int da[][string];
  string ss[string][string];
  int ii[int][int];
  initial begin
    aa["x"][1] = 10;
    aa["x"][2] = 20;
    aa["y"][7] = 30;
    $display("%0d %0d %0d", aa["x"][1], aa["y"][7], aa["z"][0]);
    $display("%0d %0d", aa.num(), aa["x"].num());
    $display("%0d", aa["x"].exists(2));
    aa["x"].delete(2);
    $display("%0d", aa["x"].exists(2));
    foreach (aa[k, j]) $display("%s %0d %0d", k, j, aa[k][j]);
    foreach (aa["x"][j]) $display("%0d", j);
    qa.push_back('{5: 1});
    qa[0][6] = 2;
    $display("%0d %0d", qa[0][5], qa[0][6]);
    $display("%0d", $size(qa[0]));
    da = new[2];
    da[1]["s"] = 4;
    ss["a"]["b"] = "v";
    $display("%s %0d", ss["a"]["b"], ss.num());
    ii[1][2] = 3;
    ii[1][2]++;
    ii[1][3] += 3;
    $display("%0d %0d %0d", ii[1][2], ii[1][3], ii.num());
    $finish;
  end
endmodule
