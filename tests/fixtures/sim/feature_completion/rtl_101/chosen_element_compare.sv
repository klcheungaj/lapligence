// Whole elements of records with a `string` or `real` member array compare
// member by member, at any size of the array (SV 11.2.2): elements of a fixed
// array, a dynamic array, a queue and an associative array.
`ifndef N
`define N 16
`endif
typedef struct { int tag; string a [`N]; real d [`N]; } rec_t;

module tb;
  rec_t ar [2];
  rec_t da [];
  rec_t qu [$];
  rec_t aa [int];
  rec_t e;
  int k = 1;

  initial begin
    e.tag = 5;
    e.a[`N - 1] = "last";
    e.d[2] = 1.5;
    ar[0] = e;
    ar[1] = e;
    $display("fixed %0d %0d", ar[0] == ar[1], ar[0] != ar[k]);
    ar[1].a[`N - 1] = "other";
    $display("fixed %0d %0d", ar[0] == ar[k], ar[0] != ar[1]);
    da = new[2];
    da[0] = e;
    da[1] = e;
    $display("dynamic %0d", da[0] == da[k]);
    da[1].d[2] = 2.5;
    $display("dynamic %0d %0d", da[0] == da[k], da[1] == e);
    qu.push_back(e);
    qu.push_back(e);
    $display("queue %0d", qu[0] == qu[k]);
    qu[1].tag = 6;
    $display("queue %0d", qu[0] == qu[k]);
    aa[3] = e;
    aa[4] = e;
    $display("assoc %0d %0d", aa[3] == aa[4], aa[3] == e);
    aa[4].a[0] = "x";
    $display("assoc %0d", aa[3] == aa[4]);
    $finish(0);
  end
endmodule
