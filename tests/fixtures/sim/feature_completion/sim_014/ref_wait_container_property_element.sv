// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_014/ref_wait_container_property_element.sv
// FIN-001 R2: a task waiting on a `ref int` formal bound to an element of a
// fixed-array, queue or dynamic-array class property, or to a member of a
// record element of one, resumes on a direct store into that element, also
// through a virtual-interface method; a store into another element does not
// resume it (SV 13.5.2, 9.4.2). Run with `-D N=3` and `-D N=16`.
`timescale 1ns/1ns
`ifndef N
`define N 3
`endif
typedef struct { int a; bit [3:0] b; int arr[`N]; } p_t;
typedef struct { int a; string s; int arr[`N]; } s_t;
class C;
  int fa[4];
  int q[$];
  int d[];
  p_t pa[3];
  s_t sa[2];
  s_t sq[$];
  function new();
    q = '{0, 0, 0};
    d = new[3];
    sq.push_back(sa[0]);
    sq.push_back(sa[0]);
  endfunction
endclass
interface bus;
  task automatic w(ref int x, input string tag);
    @(x);
    $display("%0t vif %s woke %0d", $time, tag, x);
  endtask
endinterface
module tb;
  bus b1();
  virtual bus v = b1;
  C c = new;
  int i = 2;
  int k = 1;
  task automatic w(ref int x, input string tag);
    @(x);
    $display("%0t %s woke %0d", $time, tag, x);
  endtask
  initial begin
    fork
      w(c.fa[i], "fa");
      begin #1 c.fa[1] = 9; #1 c.fa[2] = 2; end
    join
    fork
      w(c.q[i], "q");
      begin #1 c.q[0] = 9; c.q.push_back(4); #1 c.q[2] = 3; end
    join
    fork
      w(c.d[i], "d");
      begin #1 c.d[0] = 9; #1 c.d[2] = 4; end
    join
    fork
      w(c.pa[i].a, "pa");
      begin #1 c.pa[2].b = 1; #1 c.pa[2].a = 5; end
    join
    fork
      w(c.sa[k].arr[`N - 1], "sa");
      begin #1 c.sa[k].s = "x"; #1 c.sa[1].arr[`N - 1] = 6; end
    join
    fork
      w(c.sq[k].a, "sq");
      #1 c.sq[1].a = 7;
    join
    fork
      v.w(c.q[i], "q");
      #1 c.q[2] = 8;
    join
    fork
      v.w(c.sa[k].a, "sa");
      #1 c.sa[1].a = 9;
    join
    $display("done");
    $finish(0);
  end
endmodule
