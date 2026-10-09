// Class storage read inside functions and methods that a wait condition or
// event expression calls (IEEE 1800-2009 9.4.2, 9.4.3). Each waiter keeps
// its own log, so same-time wakes cannot reorder the output.
module tb;
  int gv;

  class N;
    int x;
  endclass

  class C;
    int x;
    N n;
    static int sx;
    function new();
      n = new;
    endfunction
    function int get();
      return x;
    endfunction
    function int nx();
      return n.x;
    endfunction
    function int gvx();
      return gv + x;
    endfunction
  endclass

  C h, k, old;
  string l_fx, l_at_fx, l_fnx, l_fget, l_fwrap, l_nx, l_gvx, l_global, l_static;

  function automatic int fx(C c);
    return c.x;
  endfunction
  function automatic int fnx(C c);
    return c.n.x;
  endfunction
  function automatic int fget(C c);
    return c.get();
  endfunction
  function automatic int fwrap(C c);
    return fx(c);
  endfunction
  function automatic int fglobal();
    return k.x;
  endfunction
  function automatic int fstatic();
    return C::sx;
  endfunction

  initial begin
    h = new;
    k = new;
    fork
      begin
        wait (fx(h) == 1);
        l_fx = $sformatf("%s %0t", l_fx, $time);
      end
      forever begin
        @(fx(h));
        l_at_fx = $sformatf("%s %0t", l_at_fx, $time);
      end
      begin
        wait (fnx(h) == 2);
        l_fnx = $sformatf("%s %0t", l_fnx, $time);
      end
      begin
        wait (fget(h) == 3);
        l_fget = $sformatf("%s %0t", l_fget, $time);
      end
      begin
        wait (fwrap(h) == 4);
        l_fwrap = $sformatf("%s %0t", l_fwrap, $time);
      end
      begin
        wait (h.nx() == 5);
        l_nx = $sformatf("%s %0t", l_nx, $time);
      end
      begin
        wait (h.gvx() == 16);
        l_gvx = $sformatf("%s %0t", l_gvx, $time);
      end
      begin
        wait (fglobal() == 7);
        l_global = $sformatf("%s %0t", l_global, $time);
      end
      begin
        wait (fstatic() == 8);
        l_static = $sformatf("%s %0t", l_static, $time);
      end
    join_none
    #1 h.x = 1;
    #1 h.n.x = 2;
    #1 h.x = 3;
    #1 h.x = 4;
    #1 h.n.x = 5;
    #1 gv = 12;
    #1 k.x = 7;
    #1 C::sx = 8;
    #1 begin
      old = h;
      h = new;
      h.x = 4;
    end
    #1 old.x = 20;
    #1 h.x = 11;
    #1;
    $display("fx_wait:%s", l_fx);
    $display("fx_event:%s", l_at_fx);
    $display("fnx:%s", l_fnx);
    $display("fget:%s", l_fget);
    $display("fwrap:%s", l_fwrap);
    $display("method_nx:%s", l_nx);
    $display("method_gvx:%s", l_gvx);
    $display("global_handle:%s", l_global);
    $display("static_property:%s", l_static);
    $finish(0);
  end
endmodule
