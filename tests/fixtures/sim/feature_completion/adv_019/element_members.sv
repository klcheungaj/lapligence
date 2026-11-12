// ADV-019 members of structure values and fixed-size class properties: the
// VPI application element_members.c lists, reads, finds and writes the
// array and container members of the structure elements of a fixed-size
// array and a queue, the fixed-size array and structure properties of a
// class object, and the member selects llg refuses (IEEE 1800-2009 37.17,
// 37.24, 37.29, 38.15). See readme.md for the expected output.
`timescale 1ns/1ns
typedef struct {
  int k;
  int fa[2];
  string s;
} pr_t;
typedef struct {
  int k;
  logic [7:0] b;
} pe_t;
class C;
  int fa[2];
  string sa[2];
  logic [3:0] la[2][2];
  pr_t pr;
  pe_t pe;
  function new();
    pr.k = 1;
    pr.fa[1] = 2;
    pr.s = "r";
    pe.k = 5;
    fa[0] = 10;
    fa[1] = 11;
    sa[0] = "p";
    sa[1] = "q";
    la[1][0] = 4'h9;
  endfunction
endclass
typedef struct {
  int k;
  int fa[2];
  int q[$];
  string s;
  string ss[2];
  logic [3:0] m[1:0][3];
} er_t;
module tb;
  er_t ea[2];
  er_t eq[$];
  C c;
  initial begin
    c = new;
    ea[1].k = 2;
    ea[1].fa[0] = 20;
    ea[1].fa[1] = 21;
    ea[1].q.push_back(22);
    ea[1].s = "e1";
    ea[1].ss[1] = "x";
    ea[1].m[0][2] = 4'h5;
    eq.push_back(ea[1]);
    $emembers;
    $display("ea[1].fa=%0d %0d q[0]=%0d ss[1]=%s m[1][0]=%h eq[0].fa[1]=%0d eq[0].q[0]=%0d",
             ea[1].fa[0], ea[1].fa[1], ea[1].q[0], ea[1].ss[1], ea[1].m[1][0], eq[0].fa[1],
             eq[0].q[0]);
    $display("c.fa=%0d %0d c.sa[1]=%s c.la[0][1]=%h c.pr.k=%0d c.pr.fa[0]=%0d c.pe.k=%0d", c.fa[0],
             c.fa[1], c.sa[1], c.la[0][1], c.pr.k, c.pr.fa[0], c.pe.k);
    $finish;
  end
endmodule
