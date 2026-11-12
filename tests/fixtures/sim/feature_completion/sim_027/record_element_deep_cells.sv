// FIN-001 R2: a cell of a member array of a record that is an element of a
// member array of a container element (`oq[0].e[1].arr[2]`,
// `oa[1]["a"].e[1].t[2]`), read and written with constant, run-time and mixed
// indices, also through bit and part selects, and a whole record stored in
// such a cell (`ra[1]["a"].e[1].t[2] = rv`). Run with `-D N=3` and `-D N=16`:
// the string-member records then take both storage forms.
`ifndef N
`define N 3
`endif
typedef struct { int arr[4]; } e_t;
typedef struct { e_t e[2]; } o_t;
typedef struct { int t[3]; } e2_t;
typedef struct { e2_t e[2]; } o2_t;
typedef struct { int t[3]; string s; int big[`N]; } e3_t;
typedef struct { e3_t e[2]; } s_t;
typedef struct { int a; string s; int big[`N]; } r_t;
typedef struct { r_t t[3]; } e4_t;
typedef struct { e4_t e[2]; } o4_t;
module tb;
  o_t oq[$];
  o_t od[];
  o_t oaa[string];
  o_t o;
  o2_t oa[int][string];
  o2_t o2;
  s_t sq[$];
  s_t s;
  o4_t ra[int][string];
  o4_t o4;
  r_t rv, rw;
  int i0 = 0, i1 = 1, i2 = 2;
  initial begin
    o.e[1].arr[2] = 42;
    oq.push_back(o);
    $display("oq[0].e[1].arr[2] %0d", oq[0].e[1].arr[2]);
    oq[0].e[1].arr[2] = 43;
    $display("after write %0d", oq[0].e[1].arr[2]);
    oq[i0].e[i1].arr[i2] = 44;
    $display("run-time %0d %0d", oq[i0].e[i1].arr[i2], oq[0].e[1].arr[2]);
    oq[i0].e[1].arr[2] = 45;
    $display("outer run-time %0d %0d", oq[i0].e[1].arr[2], oq[0].e[i1].arr[2]);
    oq[0].e[1].arr[i2]++;
    $display("increment %0d", oq[0].e[1].arr[2]);
    oq[0].e[1].arr[2][0] = 0;
    oq[0].e[1].arr[2][7:4] = 4'h5;
    $display("bits %0d %0d %0d", oq[0].e[1].arr[2], oq[0].e[1].arr[2][1], oq[0].e[1].arr[2][7:4]);
    $display("other cells %0d %0d %0d", oq[0].e[0].arr[2], oq[0].e[1].arr[1], oq[0].e[1].arr[3]);
    od = new[1];
    od[0] = o;
    od[0].e[1].arr[3] = 46;
    $display("dynamic %0d %0d", od[0].e[1].arr[2], od[0].e[1].arr[3]);
    oaa["k"] = o;
    oaa["k"].e[0].arr[0] = 47;
    $display("associative %0d %0d", oaa["k"].e[0].arr[0], oaa["k"].e[1].arr[2]);
    o2.e[1].t[2] = 7;
    oa[1]["a"] = o2;
    $display("oa[1][a].e[1].t[2] %0d", oa[1]["a"].e[1].t[2]);
    oa[1]["a"].e[1].t[2] = 8;
    $display("after write %0d", oa[1]["a"].e[1].t[2]);
    oa[i1]["a"].e[i1].t[i2] = 9;
    $display("run-time %0d %0d", oa[i1]["a"].e[i1].t[i2], oa[1]["a"].e[1].t[2]);
    oa[1]["a"].e[0].t[1] = 10;
    $display("nested cells %0d %0d %0d", oa[1]["a"].e[0].t[1], oa[1]["a"].e[1].t[1], oa[1].size());
    s.e[1].t[2] = 5;
    s.e[1].big[2] = 6;
    s.e[1].s = "six";
    sq.push_back(s);
    $display("string record %0d %0d %s", sq[0].e[1].t[2], sq[0].e[1].big[2], sq[0].e[1].s);
    sq[0].e[1].t[2] = 11;
    sq[0].e[1].big[2] = 12;
    sq[i0].e[i1].big[i2 - 1] = 13;
    $display("string record writes %0d %0d %0d %s", sq[0].e[1].t[2], sq[0].e[1].big[2],
             sq[0].e[1].big[1], sq[0].e[1].s);
    rv.a = 3;
    rv.s = "three";
    rv.big[1] = 4;
    ra[1]["a"] = o4;
    ra[1]["a"].e[1].t[2] = rv;
    rw = ra[1]["a"].e[1].t[2];
    $display("whole record %0d %s %0d", rw.a, rw.s, rw.big[1]);
    $display("its members %0d %s %0d", ra[1]["a"].e[1].t[2].a, ra[1]["a"].e[1].t[2].s,
             ra[1]["a"].e[1].t[2].big[1]);
    ra[1]["a"].e[1].t[2].s = "x";
    ra[i1]["a"].e[i1].t[i2].a = 9;
    $display("member writes %0d %s", ra[1]["a"].e[1].t[2].a, ra[1]["a"].e[1].t[2].s);
    $finish;
  end
endmodule
