// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_014/callee_wait_record_property.sv
// Waits whose condition calls a function or method that reads a member of an
// unpacked structure property: through the receiver, through a class-handle
// formal and through a function that calls another (SV 9.4.2, decision B2).
module tb;
  timeunit 1ns; timeprecision 1ns;
  typedef struct {
    int a;
    string s;
  } rec_t;
  class C;
    rec_t r;
    function bit ready();
      return r.s == "go";
    endfunction
  endclass
  function automatic bit has(C c, int v);
    return c.r.a == v;
  endfunction
  function automatic bit both(C c);
    return has(c, 7) && c.ready();
  endfunction
  C h = new;
  int ready_at = -1, has_at = -1, both_at = -1, event_at = -1;
  initial begin
    fork
      begin wait (h.ready()); ready_at = $time; end
      begin wait (has(h, 7)); has_at = $time; end
      begin wait (both(h)); both_at = $time; end
      begin @(has(h, 7)); event_at = $time; end
      begin
        #1 h.r.a = 7;
        #1 h.r.s = "go";
      end
    join
    $display("ready=%0d has=%0d both=%0d event=%0d", ready_at, has_at, both_at, event_at);
    $finish(0);
  end
endmodule
