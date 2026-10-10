// A queue class property in a wait condition: per-object containers
// publish no change llg can wait on.
module tb;
  class C;
    int q[$];
  endclass
  C h = new;
  initial begin
    wait (h.q.size() == 1);
    $display("woke");
  end
  initial #1 h.q.push_back(3);
endmodule
