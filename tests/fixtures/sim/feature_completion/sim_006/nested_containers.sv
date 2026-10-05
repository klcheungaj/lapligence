// SIM-006 A01: containers of containers (SV 7.5, 7.10, 7.12). An inner
// value may be written from another dynamic array, a queue, an assignment
// pattern or an unpacked concatenation; each element owns its copy.
module tb;
  int q[$][$];
  int d[][];
  string sq[$][$];
  int a[int][$];
  int src[];
  int total;
  initial begin
    q.push_back({1, 2});
    q.push_back({3});
    q.push_front('{4, 5, 6});
    q.insert(1, {});
    src = new[2];
    src[0] = 7;
    src[1] = 8;
    q.push_back(src);
    src[0] = 70;
    q[1] = {9, 9, 9, 9};
    $display("q %0d | %0d %0d %0d %0d %0d | %0d %0d", q.size(), q[0].size(), q[1].size(),
             q[2].size(), q[3].size(), q[4].size(), q[4][0], q[0][2]);
    d = new[2];
    d[0] = new[3];
    d[1] = '{5, 6};
    sq.push_back({"x"});
    sq.push_back({"y", "z"});
    a[10] = {1, 2, 3};
    a[-1] = {1, 2};
    total = 0;
    for (int i = 0; i < q.size(); i++) total += q[i].size();
    $display("d %0d %0d %0d | sq %s %s %0d | a %0d %0d %0d | total %0d", d[0].size(), d[1][1],
             d[0][2], sq[0][0], sq[1][1], sq[1].size(), a[10].size(), a[-1][1], a.num(),
             total);
    $finish(0);
  end
endmodule
