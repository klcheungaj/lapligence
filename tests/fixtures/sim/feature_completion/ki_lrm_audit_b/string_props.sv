// String and class-handle properties in wait conditions and event
// expressions (IEEE 1800-2009 9.4.2, 9.4.3). Each waiter keeps its own log,
// so same-time wakes of different processes cannot reorder the output.
module tb;
  class C;
    string s;
    C nxt;
  endclass

  C h, a, b;
  string l_wait, l_at_s, l_nxt_wait, l_at_nxt, l_len;

  initial begin
    a = new;
    b = new;
    a.s = "x";
    b.s = "y";
    h = a;
    fork
      begin
        wait (h.s == "go");
        l_wait = $sformatf("%s %0t", l_wait, $time);
      end
      forever begin
        @(h.s);
        l_at_s = $sformatf("%s %0t", l_at_s, $time);
      end
      begin
        wait (h.nxt != null);
        l_nxt_wait = $sformatf("%s %0t", l_nxt_wait, $time);
      end
      forever begin
        @(h.nxt);
        l_at_nxt = $sformatf("%s %0t", l_at_nxt, $time);
      end
      begin
        wait (h.s.len() == 4);
        l_len = $sformatf("%s %0t", l_len, $time);
      end
    join_none
    #1 a.s = "x";
    #1 b.s = "zz";
    #1 h = b;
    #1 h.s = "go";
    #1 a.s = "go!!";
    #1 h.nxt = a;
    #1 h.s = "four";
    #1 begin
      h.s = "four";
      h.nxt = a;
    end
    #1 h = a;
    #1;
    $display("wait_go:%s", l_wait);
    $display("at_s:%s", l_at_s);
    $display("nxt_wait:%s", l_nxt_wait);
    $display("at_nxt:%s", l_at_nxt);
    $display("len4:%s", l_len);
    $finish(0);
  end
endmodule
