// SIM-022 13.4.4: the call names the base method; the override forks. The
// handle comes from another virtual method, as a core service returns a root.
class root_t;
  virtual function int attach();
    return 0;
  endfunction
endclass
class forking_root extends root_t;
  virtual function int attach();
    fork
      #1 $display("late");
    join_none
    return 1;
  endfunction
endclass
class service;
  virtual function root_t get_root();
    forking_root r = new;
    return r;
  endfunction
endclass
class holder;
  static service cs = new;
  static int attached = cs.get_root().attach();
endclass
module tb;
  initial $display("attached=%0d", holder::attached);
endmodule
