// Targets that only std::randomize writes and state that only a constraint
// reads must keep their storage when the optimizer drops unused signals.
module tb;
  int y;
  int scalar_target;
  int dyn_target[];
  int queue_target[$];
  int ok;
  initial begin
    y = 5;
    ok = std::randomize(scalar_target) with { scalar_target > 0; scalar_target < y; };
    ok = ok & std::randomize(dyn_target) with {
      dyn_target.size() == 2;
      foreach (dyn_target[i]) dyn_target[i] == i;
    };
    ok = ok & std::randomize(queue_target) with {
      queue_target.size() inside {[1:3]};
      foreach (queue_target[i]) queue_target[i] < y;
    };
    $display("ok=%0d", ok);
    $finish;
  end
endmodule
