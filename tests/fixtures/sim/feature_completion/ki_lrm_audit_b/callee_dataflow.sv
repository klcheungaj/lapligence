// Waits on functions whose bodies read storage through handles they copy
// or receive: automatic handle locals, virtual-interface formals and
// variables, container properties and mailbox counts (IEEE 1800-2009
// 9.4.2 L11837-11840, 9.4.3).
interface bus_if;
  logic [3:0] s;
endinterface

module tb;
  class C;
    int x;
    int q[$];
    mailbox mb = new;
    function int total();
      int sum = 0;
      foreach (q[i]) sum += q[i];
      return sum;
    endfunction
  endclass
  bus_if i0();
  bus_if i1();
  virtual bus_if v = i0;
  C h = new;
  int t_alias = -1, t_chain = -1, t_vif = -1, t_vglob = -1, t_size = -1;
  int t_total = -1, t_num = -1, t_rebind = -1;
  function automatic int alias_x(C c);
    C d;
    d = c;
    return d.x;
  endfunction
  function automatic int chain_x(C c);
    C first = c;
    C second;
    second = first;
    return second.x;
  endfunction
  function automatic int vread(virtual bus_if vv);
    return int'(vv.s);
  endfunction
  function automatic int vglob();
    return int'(v.s);
  endfunction
  function automatic int qsize(C c);
    C t = c;
    return t.q.size();
  endfunction
  function automatic int mnum(C c);
    return c.mb.num();
  endfunction
  initial begin wait (alias_x(h) == 1); t_alias = int'($time); end
  initial begin wait (chain_x(h) == 2); t_chain = int'($time); end
  initial begin wait (vread(v) == 3); t_vif = int'($time); end
  initial begin wait (vglob() == 4); t_vglob = int'($time); end
  initial begin wait (qsize(h) == 1); t_size = int'($time); end
  initial begin wait (h.total() == 9); t_total = int'($time); end
  initial begin wait (mnum(h) == 1); t_num = int'($time); end
  initial begin wait (vglob() == 6); t_rebind = int'($time); end
  initial begin
    i1.s = 4'd6;
    #1 h.x = 1;
    #1 h.x = 2;
    #1 i0.s = 4'd3;
    #1 i0.s = 4'd4;
    #1 h.q.push_back(4);
    #1 h.q.push_back(5);
    #1 h.mb.put(1);
    #1 v = i1;
    #1 begin
      $display("alias=%0d chain=%0d vif=%0d vglob=%0d", t_alias, t_chain, t_vif, t_vglob);
      $display("size=%0d total=%0d num=%0d rebind=%0d", t_size, t_total, t_num, t_rebind);
    end
    $finish(0);
  end
endmodule
