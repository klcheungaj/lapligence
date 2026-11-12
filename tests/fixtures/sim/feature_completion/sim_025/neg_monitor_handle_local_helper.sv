// SIM-025: a `$monitor` whose only call is a statement-bodied function with a
// class-handle local and no write of its own. The read-only Postponed
// evaluator does not run such a body yet, so the design rejects; the same call
// beside an argument with private effects (`monitor_method_calls`) runs.
module tb;
  class C; int v; endclass
  C h;
  function automatic int peek(C c);
    C d;
    d = c;
    return d.v;
  endfunction
  initial begin
    h = new;
    $monitor("%0t peek=%0d", $time, peek(h));
    #1 h.v = 1;
    #1 $finish(0);
  end
endmodule
