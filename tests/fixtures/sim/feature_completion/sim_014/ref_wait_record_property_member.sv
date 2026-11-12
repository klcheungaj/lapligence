// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_014/ref_wait_record_property_member.sv
// FIN-001 R2: a task waiting on a `ref int` formal bound to a member of a
// record property with a string member resumes on a direct store into that
// member, on a store through another formal bound to it, through a method's
// own property, through an alias handle and through a virtual-interface
// method (SV 13.5.2, 9.4.2). Stores into other members or properties of the
// object do not resume it. Run with `-D N=3` and `-D N=16`.
`timescale 1ns/1ns
`ifndef N
`define N 3
`endif
typedef struct { int a; string s; int arr[`N]; int b; } rec_t;
interface bus;
  task automatic w(ref int x, input string tag);
    @(x);
    $display("%0t vif %s woke %0d", $time, tag, x);
  endtask
endinterface
module tb;
  class H;
    rec_t r;
    int other;
    task automatic run();
      fork
        w(r.a, "this");
        #1 r.a = 21;
      join
    endtask
    task automatic w(ref int x, input string tag);
      @(x);
      $display("%0t %s woke %0d", $time, tag, x);
    endtask
  endclass
  bus b1();
  virtual bus v = b1;
  H h = new;
  H g;
  task automatic w(ref int x, input string tag);
    @(x);
    $display("%0t %s woke %0d", $time, tag, x);
  endtask
  task automatic store(ref int x);
    #1 x = 5;
  endtask
  task automatic until9(ref int x);
    wait (x == 9);
    $display("%0t wait woke %0d", $time, x);
  endtask
  initial begin
    fork
      w(h.r.a, "member");
      begin
        #1 h.r.b = 3;
        h.other = 4;
        h.r.arr[1] = 2;
        h.r.s = "x";
        #1 h.r.a = 7;
      end
    join
    fork
      w(h.r.arr[`N - 1], "formal store");
      store(h.r.arr[`N - 1]);
    join
    fork
      until9(h.r.b);
      begin #1 h.r.b = 8; #1 h.r.b = 9; end
    join
    h.run();
    g = h;
    fork
      w(g.r.a, "alias");
      #1 h.r.a = 33;
    join
    fork
      v.w(h.r.arr[0], "member");
      #1 h.r.arr[0] = 44;
    join
    $display("done");
    $finish(0);
  end
endmodule
