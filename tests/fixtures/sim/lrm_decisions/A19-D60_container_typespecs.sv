// Decision A19-D60: the typespecs of dynamic arrays, queues and associative
// arrays (IEEE 1800-2009 37.23). Companion of A19-D60_container_typespecs.c.
`timescale 1ns/1ns
module tb;
  typedef int iq_t[$];
  typedef logic [3:0] la_t[string];
  int q[$];
  int qb[$:7];
  logic [7:0] d[];
  int as[string];
  int ai[int];
  bit [2:0] al[logic [3:0]];
  byte aw[*];
  iq_t tq;
  la_t ta;
  int mixed[$][2];
  int dd[][];
  typedef struct { int m[$]; byte n[]; } s_t;
  s_t sv;
  int fx[4];
  initial begin
    q.push_back(5);
    d = new[2];
    #1 $d60;
  end
endmodule
