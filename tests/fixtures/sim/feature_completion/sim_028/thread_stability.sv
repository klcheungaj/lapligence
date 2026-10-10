// SIM-028 A01: thread and object stability (IEEE 1800-2009 18.14.1-18.14.3).
// The suite runs this design twice, with and without EXTRA_ACTIVITY, and
// requires identical output apart from lines starting with "extra". The
// extra activity is what 18.14.1 says must not perturb existing streams: a
// new instance (its own initialization RNG), displays, and a thread added
// after the existing processes of an instance that creates objects, seeds
// them, shuffles and forks. Values themselves are implementation dependent,
// so the expected output is this relation, not numbers.
class item_c;
  int k;
endclass

`ifdef EXTRA_ACTIVITY
module worker;
  int unsigned w;
  initial begin
    w = $urandom;
    #1 $display("extra worker %0d", w);
  end
endmodule
`endif

module drawer;
  int unsigned v;
  initial v = $urandom;
endmodule

module tb;
  int unsigned main_draws[3];
  int unsigned child_draws[2];
  int unsigned after_new;
  int unsigned clocked_draws[3];
  string object_state;
  string static_state;
  item_c item;
  // A static declaration initializer seeds from its instance (18.14.1).
  item_c static_item = new;
  bit clk;
`ifdef EXTRA_ACTIVITY
  worker early();
`endif
  drawer d1();
  drawer d2();

  initial begin : main
    for (int i = 0; i < 3; i++) main_draws[i] = $urandom;
`ifdef EXTRA_ACTIVITY
    $display("extra display in main");
`endif
    fork
      child_draws[0] = $urandom;
      child_draws[1] = $urandom_range(1000);
    join
    item = new;
    object_state = item.get_randstate();
    static_state = static_item.get_randstate();
    after_new = $urandom;
    #12;
    $display("main %0d %0d %0d", main_draws[0], main_draws[1], main_draws[2]);
    $display("children %0d %0d", child_draws[0], child_draws[1]);
    $display("object %s", object_state);
    $display("static object %s", static_state);
    $display("after new %0d", after_new);
    $display("clocked %0d %0d %0d", clocked_draws[0], clocked_draws[1], clocked_draws[2]);
    $display("instances %0d %0d differ %0d", d1.v, d2.v, d1.v != d2.v);
    $finish;
  end

  always #2 clk = !clk;

  initial begin : clocked
    for (int i = 0; i < 3; i++) begin
      @(posedge clk);
      clocked_draws[i] = $urandom;
    end
  end

`ifdef EXTRA_ACTIVITY
  // Added after the existing processes of this instance.
  initial begin : extra
    int q[$];
    item_c other;
    q = '{1, 2, 3, 4};
    other = new;
    other.srandom(1);
    q.shuffle();
    repeat (2) @(posedge clk) void'($urandom);
    fork
      begin
        int z;
        z = $urandom;
      end
    join_none
    $display("extra thread %0d", q.size());
  end
  worker late();
`endif
endmodule
