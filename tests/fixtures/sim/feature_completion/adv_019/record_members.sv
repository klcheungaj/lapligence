// ADV-019 structure members: the VPI application record_members.c lists,
// reads, finds and writes the members of unpacked structures that llg keeps
// as member leaves or in columns: string, class handle, queue and dynamic
// array members, unpacked array members of packed and real cells (one leaf
// per cell, or one column), and bit and part selects of members (IEEE
// 1800-2009 37.17, 37.19, 37.24, 38.15, 38.36.1), and the unpacked array
// members of strings and structures it refuses. See readme.md for the
// expected output.
`timescale 1ns/1ns
class C;
  int v = 3;
endclass
typedef struct {
  int a;
  int arr[3];
  string s;
  int q[$];
  int da[];
  C h;
  logic [7:0] f;
  int arr20[20];
  real ra[2];
  int m[2][3];
} rec_t;
typedef struct {
  int big[5000];
  logic [7:0] w;
  real x;
} col_t;
typedef struct {
  int x;
} in_t;
typedef struct {
  int k;
  string sa[2];
  in_t ia[2];
} lr_t;
module tb;
  rec_t r;
  col_t cr;
  lr_t lr;
  initial begin
    r.h = new;
    r.a = 1;
    r.arr[0] = 5;
    r.arr[1] = 6;
    r.arr[2] = 7;
    r.s = "hi";
    r.q.push_back(3);
    r.q.push_back(4);
    r.da = new[2];
    r.da[1] = 9;
    r.f = 8'ha5;
    r.arr20[19] = 19;
    r.ra[1] = 2.5;
    r.m[1][2] = 12;
    cr.big[4999] = 42;
    cr.w = 8'hf0;
    cr.x = 1.5;
    lr.k = 4;
    lr.sa[1] = "l";
    lr.ia[1].x = 6;
    $rmembers(1);
    $display("r.arr=%0d %0d %0d s=%s q=%0d %0d da[1]=%0d f=%h arr20[19]=%0d ra[0]=%0.2f m[0][1]=%0d",
             r.arr[0], r.arr[1], r.arr[2], r.s, r.q[0], r.q[1], r.da[1], r.f, r.arr20[19],
             r.ra[0], r.m[0][1]);
    $display("cr.big[4999]=%0d cr.w=%h cr.x=%0.2f h.v=%0d lr=%0d %s %0d", cr.big[4999], cr.w, cr.x,
             r.h.v, lr.k, lr.sa[1], lr.ia[1].x);
    #1 r.arr[0] = 50;
    #1 cr.big[7] = 7;
    #1 $rmembers(2);
    $finish;
  end
  always @(r.arr[1]) $display("%0t: hdl saw r.arr[1]=%0d", $time, r.arr[1]);
endmodule
