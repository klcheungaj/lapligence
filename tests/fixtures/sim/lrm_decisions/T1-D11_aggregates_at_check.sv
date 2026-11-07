// Decision T1-D11 for aggregates and handles: a change that is undone before
// the check is no change for a cell of a fixed-size array (few or large),
// a record member, an element of an array of records, a class handle or a
// class property. Strings and the cells of queues and dynamic arrays wake on
// each change, as they always did.
//
// IEEE 1800-2009 4.5 (SystemVerilog-1800-2009.txt L3410-3411): "update the
// modified object; schedule evaluation event for any process sensitive to the
// object;" 9.4.2 L11807: "An implicit event shall be detected on any change in
// the value of the expression." llg departs from the per-write reading on
// purpose; see T1-D11_changes_at_check for the scalar cases.
//
// Expected output: from 10 to 50 only the step markers; at 51 the property
// change that is kept wakes its waiter; the `always @*` run counts stay 0;
// at 60 the string, queue cell and dynamic cell each wake once; at 70 a
// waiter on a variable index wakes for the second cell even though the first
// cell's change was undone, and at 71 not at all.
`timescale 1ns / 1ns
module tb;
  class C;
    bit [7:0] p;
  endclass
  typedef struct { bit [3:0] a; int k; } rec_t;
  bit [7:0] big [0:8191];
  bit [7:0] few [0:3];
  rec_t r;
  rec_t ra [0:2];
  C h1 = new, h2 = new, h;
  string s;
  int q[$];
  int d[];
  int idx;
  int big_runs, small_runs, rec_runs;
  bit [7:0] dyn [0:3];
  int di = 1;
  bit [7:0] big_seen, small_seen;
  bit [3:0] rec_seen;

  always @(big[5]) $display("%0t big[5]=%0d", $time, big[5]);
  always @(big[idx]) $display("%0t big[idx]=%0d", $time, big[idx]);
  always @* big_seen = big[idx + 1];
  always @(big_seen) big_runs = big_runs + 1;
  always @* small_seen = few[idx];
  always @(small_seen) small_runs = small_runs + 1;
  always @(r.a) $display("%0t r.a=%0d", $time, r.a);
  always @(ra[1].a) $display("%0t ra[1].a=%0d", $time, ra[1].a);
  always @* rec_seen = ra[idx].a;
  always @(rec_seen) rec_runs = rec_runs + 1;
  always @(h) $display("%0t h changed", $time);
  always @(h1.p) $display("%0t h1.p=%0d", $time, h1.p);
  always @(s) $display("%0t s=%s", $time, s);
  always @(q[0]) $display("%0t q[0]=%0d", $time, q[0]);
  always @(d[0]) $display("%0t d[0]=%0d", $time, d[0]);
  always @(dyn[di]) $display("%0t dyn[di]=%0d", $time, dyn[di]);

  initial begin
    q = '{0};
    d = new[1];
    #10 $display("-- 10 large array cells");
    big[5] = 1; big[5] = 0; big[6] = 2; big[6] = 0;
    #10 $display("-- 20 small array cell");
    few[0] = 3; few[0] = 0;
    #10 $display("-- 30 records");
    r.a = 4; r.a = 0; ra[1].a = 5; ra[1].a = 0; ra[0].a = 6; ra[0].a = 0;
    #10 $display("-- 40 handle");
    h = h2; h = null;
    #10 $display("-- 50 property");
    h1.p = 7; h1.p = 0;
    #1 h1.p = 9;
    #1 $display("   big_runs=%0d small_runs=%0d rec_runs=%0d", big_runs, small_runs, rec_runs);
    #8 $display("-- 60 string, queue and dynamic cells");
    s = "b"; s = "";
    q[0] = 1; q[0] = 0;
    d[0] = 1; d[0] = 0;
    #10 $display("-- 70 variable index");
    dyn[0] = 3; dyn[0] = 0; dyn[1] = 4;
    #1 dyn[2] = 5; dyn[2] = 0;
    #9 $finish(0);
  end
endmodule
