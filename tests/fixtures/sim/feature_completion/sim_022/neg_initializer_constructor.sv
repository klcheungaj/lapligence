// SIM-022-A03 nearest illegal: a class handle initializer runs a constructor
// that forks a join_none branch.
module tb;
  class Watcher;
    function new();
      fork
        #1 $display("late");
      join_none
    endfunction
  endclass
  Watcher watcher = new;
  initial $display("constructed=%0d", watcher != null);
endmodule
