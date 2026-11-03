// Queue and dynamic-array members of container record elements read in
// conditions, loop headers, `foreach`, declaration initializers and the
// value of a timed assignment: each evaluation reads the element's current
// member (SV 7.5, 7.10, 12.4-12.7, 6.21, 9.4.5).
module tb;
  typedef struct {
    string s;
    int q[$];
    string names[];
  } item_t;
  item_t list[$];
  item_t fa[2];
  item_t aa[int];
  int qq[$][$];
  int n;
  int c;
  always_comb c = fa[0].q.size();
  function automatic int total(int i);
    return qq[i].sum();
  endfunction
  initial begin
    item_t it;
    it.s = "a";
    it.q = '{1, 2, 3};
    it.names = new[2];
    it.names[0] = "x";
    it.names[1] = "y";
    list.push_back(it);
    fa[1] = it;
    aa[7] = it;
    if (list[0].q.size() == 3) $display("1 three");
    for (int j = 0; j < list[0].q.size(); j++) begin
      $display("2 %0d %0d", j, list[0].q[j]);
      if (j == 0) list[0].q.push_back(9);
    end
    foreach (fa[1].q[j]) begin
      $display("3 %0d %0d", j, fa[1].q[j]);
      fa[1].q[j] = fa[1].q[j] * 10;
    end
    $display("3 %0d %0d %0d", fa[1].q[0], fa[1].q[1], fa[1].q[2]);
    begin
      automatic int m = list[0].q.size();
      automatic string last = fa[1].names[m - 3];
      $display("4 %0d %s", m, last);
    end
    n = #1 list[0].q[3];
    $display("5 %0d %0d", n, $time);
    while (list[0].q.size() > 1) list[0].q.delete(list[0].q.size() - 1);
    $display("6 %0d %0d", list[0].q.size(), list[0].q[0]);
    case (aa[7].q.size())
      3: $display("7 three");
      default: $display("7 other");
    endcase
    if (aa[7].names[0] == "x" && aa[7].q[2] == 3) $display("8 x 3");
    n = 0;
    do n++; while (n < fa[1].q.size());
    $display("9 %0d", n);
    n = aa[7].q.size() > 2 ? aa[7].q[2] : -1;
    $display("10 %0d", n);
    qq.push_back('{1, 2, 3});
    if (qq[0].sum() > 5) $display("11 %0d", total(0));
    fa[0].q.push_back(4);
    #1 $display("12 %0d", c);
    fork
      begin
        wait (fa[0].q.size() > 1);
        $display("13 %0d %0d", fa[0].q[1], $time);
      end
      #2 fa[0].q.push_back(8);
    join
    $finish(0);
  end
endmodule
