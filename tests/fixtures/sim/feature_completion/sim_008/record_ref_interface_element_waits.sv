// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_008/record_ref_interface_element_waits.sv
// FIN-001 R2: virtual-interface methods waiting on a native record `ref`
// formal bound to an element of a queue, fixed array or dynamic array, or of
// a record queue or array class property, at constant and run-time indices
// (SV 13.5.2, 25.9). The handle selects the instance at run time; a store into
// another member or element does not resume the waiter. Run with `-D N=3`
// and `-D N=16`.
`timescale 1ns/1ns
`ifndef N
`define N 3
`endif
typedef struct { int a; string s; int arr[`N]; } r_t;
class H;
  r_t ra[2];
  r_t rq[$];
endclass
interface bus;
  int id;
  task automatic w(ref r_t r, input string tag);
    @(r.a);
    $display("%0t %s woke %0d through %0d", $time, tag, r.a, id);
  endtask
endinterface
module tb;
  bus b1();
  bus b2();
  virtual bus v;
  r_t q[$];
  r_t fa[2];
  r_t d[];
  H o = new;
  int i = 1;
  initial begin
    b1.id = 1;
    b2.id = 2;
    q.push_back(fa[0]);
    q.push_back(fa[0]);
    d = new[2];
    o.rq.push_back(fa[0]);
    o.rq.push_back(fa[0]);
    v = b1;
    fork
      v.w(q[0], "q[0]");
      begin #1 q[0].s = "x"; q[1].a = 9; #1 q[0].a = 1; end
    join
    v = b2;
    fork
      v.w(q[i], "q[i]");
      #1 q[1].a = 2;
    join
    fork
      v.w(fa[i], "fa[i]");
      begin #1 fa[0].a = 9; #1 fa[1].a = 3; end
    join
    v = b1;
    fork
      v.w(d[i], "d[i]");
      #1 d[1].a = 4;
    join
    fork
      v.w(o.ra[i], "o.ra[i]");
      begin #1 o.ra[1].arr[0] = 9; #1 o.ra[1].a = 5; end
    join
    fork
      v.w(o.rq[i], "o.rq[i]");
      #1 o.rq[1].a = 6;
    join
    $display("done");
    $finish(0);
  end
endmodule
