class C;
  int v;
  function new(int x = 0);
    v = x;
  endfunction
  function C me();
    return this;
  endfunction
  function C make(int x);
    C c = new(x);
    return c;
  endfunction
  function int take(C x);
    return x.v + v;
  endfunction
  function void log(C x);
    $display("log %0d", x.v);
  endfunction
  function void from_method();
    log(me());
    log(make(2));
    $display("%0d", take(make(3)));
  endfunction
  task from_task();
    log(me());
    #1;
    log(make(4));
    $display("%0d", take(make(5).me()));
  endtask
endclass
module tb;
  function automatic C build(int x);
    C c = new(x);
    return c;
  endfunction
  function automatic int read(C x);
    return x.v;
  endfunction
  function automatic int nested(int x);
    return read(build(x));
  endfunction
  function automatic C wrap(C x);
    return x;
  endfunction
  C c = new(1);
  initial begin
    c.from_method();
    c.from_task();
    $display("%0d", nested(6));
    $display("%0d", read(wrap(wrap(build(7)))));
    $finish;
  end
endmodule
