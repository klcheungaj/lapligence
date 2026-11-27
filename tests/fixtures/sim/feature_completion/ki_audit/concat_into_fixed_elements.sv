// Fixed-array elements of queues, dynamic and associative arrays take the
// sources of a fixed-array variable: an unpacked array concatenation with an
// item of a run-time size, a queue or dynamic array, and a concatenation of
// slices, with the run-time size check of a fixed target.
module tb;
  int q[$] = '{1, 2};
  int fa[int][3];
  int da[][3];
  int qa[$][3];
  int sa[string][3];
  int f[3];
  real rq[$] = '{1.5, 2.5};
  real ra[int][2];
  initial begin
    fa[1] = {q, 0};
    fa[2] = {7, q};
    $display("fa[1]=%p fa[2]=%p", fa[1], fa[2]);
    da = new[2];
    da[1] = {q, 5};
    $display("da=%p", da);
    qa.push_back('{0, 0, 0});
    qa[0] = {3, q};
    $display("qa=%p", qa);
    sa["k"] = {q, 9};
    $display("sa=%p", sa);
    q.push_back(4);
    fa[3] = q;
    $display("fa[3]=%p", fa[3]);
    f = {q[0:1], 5};
    fa[4] = {f[1:2], 6};
    $display("fa[4]=%p", fa[4]);
    ra[1] = rq;
    $display("ra[1]=%p", ra[1]);
  end
endmodule
