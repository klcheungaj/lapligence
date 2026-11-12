// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_008/record_ref_wait_native_elements.sv
// FIN-001 R2: tasks waiting on `@(r.a)` of a record `ref` formal bound to an
// element of a queue, fixed array or dynamic array of records with a string
// member (SV 13.5.2, 9.4.2). With `-D N=3` each resumes at the store into its
// element's `a`; a store into another member or element resumes nothing.
// With `-D N=16` the record type has the column form, whose formal takes such
// an element by address with no marker, and the call rejects at its location.
`timescale 1ns/1ns
`ifndef N
`define N 3
`endif
typedef struct { int a; string s; int arr[`N]; } r_t;
module tb;
  r_t q[$];
  r_t fa[2];
  r_t d[];
  int i = 1;
  task automatic w(ref r_t r, input string tag);
    @(r.a);
    $display("%0t %s woke %0d", $time, tag, r.a);
  endtask
  initial begin
    q.push_back(fa[0]);
    q.push_back(fa[0]);
    d = new[2];
    fork
      w(q[i], "q[i]");
      begin #1 q[1].s = "x"; q[0].a = 9; #1 q[1].a = 1; end
    join
    fork
      w(fa[i], "fa[i]");
      begin #1 fa[1].arr[0] = 9; #1 fa[1].a = 2; end
    join
    fork
      w(d[i], "d[i]");
      #1 d[1].a = 3;
    join
    $display("done");
    $finish(0);
  end
endmodule
