// SIM-015: process handles as input, output, inout, ref and const ref formals,
// as function results and as formals of a static task (SV 9.7, 13.3-13.5).
module tb;
  process g;
  int stage;

  task automatic by_value(input process p, output int st);
    st = p.status();
  endtask

  task automatic give(output process p);
    p = g;
  endtask

  task automatic swap_null(inout process p, output int was_waiting);
    was_waiting = p != null && p.status() == process::WAITING;
    p = null;
  endtask

  task automatic retarget(ref process p);
    p = process::self();
  endtask

  task automatic kill_ref(ref process p);
    p.kill();
  endtask

  function automatic process same(process p);
    return p;
  endfunction

  function automatic bit is_live(const ref process p);
    return p.status() != process::FINISHED && p.status() != process::KILLED;
  endfunction

  task remember(input process p);
    #0;
    $display("static %s", p.status().name());
  endtask

  initial begin
    process a, b, c;
    int st, w;
    fork
      begin g = process::self(); #100; stage = 1; end
    join_none
    #1;
    by_value(g, st);
    $display("by_value %0d", st);
    give(a);
    $display("output %0d", a == g);
    b = g;
    swap_null(b, w);
    $display("inout %0d %0d", w, b == null);
    retarget(c);
    $display("ref %0d %s", c == process::self(), c.status().name());
    c = same(g);
    $display("return %0d", c == g);
    $display("const_ref %0d", is_live(c));
    remember(g);
    kill_ref(c);
    $display("killed %s %s", g.status().name(), a.status().name());
    $display("const_ref %0d stage %0d", is_live(g), stage);
    $finish;
  end
endmodule
