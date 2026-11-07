// SIM-022 13.4.4: every deferred statement here is reachable from a static
// initializer and none executes under one: a false guard, an early return, an
// override the object does not have, and a call the initializer never makes.
// The same functions then run from a process, where they are legal.
class base;
  virtual function int attach(int go);
    return 0;
  endfunction
endclass
class forking extends base;
  virtual function int attach(int go);
    fork
      #1 $display("late attach");
    join_none
    return 1;
  endfunction
endclass
module tb;
  int flag = 0;
  event e;
  function automatic int guarded(int go);
    if (go > 0) begin
      fork
        #1 $display("late guarded %0d", go);
      join_none
    end
    return go;
  endfunction
  function automatic int early(int go);
    if (go == 0) return 7;
    flag <= go;
    ->> e;
    return go;
  endfunction
  function automatic base make(int go);
    base b = new;
    forking f = new;
    if (go > 0) return f;
    return b;
  endfunction
  int a = guarded(0);
  int b = early(0);
  base c = make(0);
  int d = c.attach(0);
  initial begin
    $display("%0d %0d %0d", a, b, d);
    a = guarded(1);
    fork
      @(e) $display("event");
    join_none
    #0;
    b = early(2);
    c = make(1);
    d = c.attach(1);
    #2 $display("%0d %0d %0d %0d", a, b, d, flag);
    $finish;
  end
endmodule
