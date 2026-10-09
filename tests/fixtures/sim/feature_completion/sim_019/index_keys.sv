// SIM-019: index-returning methods keep the receiver's index type and
// declaration order (SV 7.12.1, 7.12.4): signed and narrow associative keys,
// string keys (a string queue result), fixed arrays with ascending,
// descending and offset ranges (declared indices, also for `item.index`),
// and dynamic arrays and queues (positions).
module tb;
  int fa[6] = '{5, 3, 9, 3, 1, 7};
  byte fd[4:1] = '{8'd4, 8'd2, 8'd4, 8'd9};
  shortint fo[-2:1] = '{16'sd8, -16'sd3, 16'sd8, 16'sd0};
  real fr[3] = '{2.5, -1.0, 2.5};
  int q[$];
  byte bq[$];
  shortint sq[$];
  real rq[$];
  string skeys[$];
  int sa[string];
  logic [3:0] na[byte];
  byte bkeys[$];
  byte bk;
  int ka[int];
  int kq[$];
  int dyn[];
  int vals[$];

  initial begin
    q = fa.find_index with (item == 3);
    $display("fa %0d %0d %0d", q.size(), q[0], q[1]);
    q = fa.find_last_index with (item.index < 3);
    $display("fa-last %0d", q[0]);
    q = fa.find with (item > 3);
    $display("fa-find %0d %0d %0d %0d", q.size(), q[0], q[1], q[2]);
    q = fa.min();
    $display("fa-min %0d", q[0]);
    q = fa.unique();
    $display("fa-unique %0d", q.size());

    // fd[4]=4, fd[3]=2, fd[2]=4, fd[1]=9.
    q = fd.find_index with (item == 4);
    $display("fd %0d %0d %0d", q.size(), q[0], q[1]);
    q = fd.find_first_index with (item.index == 2);
    $display("fd-index %0d %0d", q.size(), q[0]);
    bq = fd.max();
    $display("fd-max %0d", bq[0]);
    // 7.12.1: unique_index order is unrelated to the receiver's and a
    // duplicate may report any of its indices, so print the sorted values
    // the indices select.
    q = fd.unique_index();
    vals.delete();
    foreach (q[i]) vals.push_back(fd[q[i]]);
    vals.sort();
    $display("fd-unique %0d %0d %0d %0d", q.size(), vals[0], vals[1], vals[2]);

    // fo[-2]=8, fo[-1]=-3, fo[0]=8, fo[1]=0.
    q = fo.find_index with (item >= 16'sd0);
    $display("fo %0d %0d %0d %0d", q.size(), q[0], q[1], q[2]);
    sq = fo.min();
    $display("fo-min %0d", sq[0]);
    q = fo.find_last_index with (item == 8);
    $display("fo-last %0d", q[0]);

    q = fr.unique_index();
    rq.delete();
    foreach (q[i]) rq.push_back(fr[q[i]]);
    rq.sort();
    $display("fr %0d %0.1f %0.1f", q.size(), rq[0], rq[1]);
    rq = fr.min();
    $display("fr-min %0.1f", rq[0]);

    sa["delta"] = 4;
    sa["alpha"] = 1;
    sa["charlie"] = 4;
    sa["bravo"] = 2;
    skeys = sa.find_index with (item == 4);
    $display("sa %0d %s %s", skeys.size(), skeys[0], skeys[1]);
    skeys = sa.find_first_index with (item > 1);
    $display("sa-first %s", skeys[0]);
    skeys = sa.find_last_index with (item < 4);
    $display("sa-last %s", skeys[0]);
    skeys = sa.unique_index();
    vals.delete();
    foreach (skeys[i]) vals.push_back(sa[skeys[i]]);
    vals.sort();
    $display("sa-unique %0d %0d %0d %0d", skeys.size(), vals[0], vals[1], vals[2]);
    skeys = sa.find_index with (item.index().len() == 5);
    $display("sa-keylen %0d %s %s %s", skeys.size(), skeys[0], skeys[1], skeys[2]);

    bk = -8'sd5;
    na[bk] = 4'd1;
    bk = 8'sd100;
    na[bk] = 4'd1;
    bk = 8'sd0;
    na[bk] = 4'd2;
    bkeys = na.find_index with (item == 4'd1);
    $display("na %0d %0d %0d", bkeys.size(), bkeys[0], bkeys[1]);

    ka[30] = 1;
    ka[-30] = 2;
    ka[0] = 3;
    kq = ka.find_index with (item.index != 0);
    $display("ka %0d %0d %0d", kq.size(), kq[0], kq[1]);

    dyn = new[4];
    dyn = '{7, 8, 7, 9};
    q = dyn.find_index with (item == 7);
    $display("dyn %0d %0d %0d", q.size(), q[0], q[1]);
    q = dyn.unique_index();
    vals.delete();
    foreach (q[i]) vals.push_back(dyn[q[i]]);
    vals.sort();
    $display("dyn-unique %0d %0d %0d %0d", q.size(), vals[0], vals[1], vals[2]);
    $finish(0);
  end
endmodule
