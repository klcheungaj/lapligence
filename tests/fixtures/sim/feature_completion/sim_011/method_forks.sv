// SIM-011: fork branches in class methods use the receiver, implicitly or
// as explicit `this`, for property access and method calls across join
// kinds and nested forks (SV 8.11, 9.3.2).
module tb;
  class base;
    int n;
    virtual function int scale(); return 1; endfunction
    task bump(int by); #1 n += by * scale(); endtask
  endclass

  class worker extends base;
    int id;
    function new(int i); id = i; endfunction
    virtual function int scale(); return 10; endfunction
    task run();
      fork
        begin this.bump(1); fork #1 this.n++; join end
        begin #5 $display("seen %0d", n); end
      join_any
      $display("any %0d at %0d", n, $time);
      wait fork;
      $display("all %0d at %0d", this.n, $time);
    endtask
    task detached();
      fork begin #1 n = id * 10; $display("branch %0d", id); end join_none
      #2 $display("detached %0d n %0d", id, n);
    endtask
    task joined();
      fork #1 n++; #2 bump(1); join
      $display("joined %0d at %0d", n, $time);
    endtask
  endclass

  initial begin
    worker w = new(3);
    w.run();
    w.detached();
    w.joined();
    $finish;
  end
endmodule
