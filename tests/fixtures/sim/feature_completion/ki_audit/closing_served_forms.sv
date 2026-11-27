// Forms that known-issues sections listed as rejected and that run:
// unpacked array concatenation items that are elements of containers,
// members of packed structure elements of class-property containers, and
// rows of a two-dimensional array of interface instances as values.
interface ifc;
  int n;
endinterface

module tb;
  typedef struct packed {
    logic [3:0] hi, lo;
  } p_t;

  class c;
    p_t q[$];
    p_t da[];
    p_t aa[string];
  endclass

  int qq[$][$];
  int q[$];
  int d[];
  int f3[3];
  int s[$] = '{7};
  int t[$] = '{6, 8};
  int aq[int][$];
  int m[2][2];
  c h;
  ifc i[2][2]();
  virtual ifc v[2];
  virtual ifc g[2][2];

  initial begin
    qq.push_back('{1});
    qq.push_back('{2, 3});
    q = {qq[1], 0};
    d = {qq[1], 4};
    f3 = {qq[1], 5};
    $display("q %0d %0d %0d %0d", q.size(), q[0], q[1], q[2]);
    $display("d %0d %0d %0d %0d", d.size(), d[0], d[1], d[2]);
    $display("f3 %0d %0d %0d", f3[0], f3[1], f3[2]);
    qq = {qq[1], s};
    $display("qq %0d %0d %0d %0d", qq.size(), qq[0].size(), qq[0][1], qq[1][0]);
    aq[1] = {s, 8};
    $display("aq %0d %0d %0d", aq[1].size(), aq[1][0], aq[1][1]);
    m = {t, t};
    $display("m %0d %0d %0d", m[0][0], m[0][1], m[1][1]);

    h = new;
    h.q.push_back(8'h56);
    h.q[0].hi = 4'h9;
    h.da = new[2];
    h.da[1] = 8'h00;
    h.da[1].lo = 4'h5;
    h.aa["x"] = 8'h21;
    h.aa["x"].hi++;
    $display("p %h %h %h %h", h.q[0], h.q[0].lo, h.da[1], h.aa["x"]);

    i[1][0].n = 4;
    i[1][1].n = 5;
    v = i[1];
    g[0] = i[1];
    $display("v %0d %0d %0d %0d", v[0].n, v[1].n, g[0][0].n, g[0][1].n);
    $finish(0);
  end
endmodule
