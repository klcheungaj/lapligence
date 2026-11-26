// A wait holds the event it started on: assignments to the event variable do
// not change it, whatever process waits for the control.
`timescale 1ns / 1ns

interface evi;
  event ie;
endinterface

module tb;
  class c;
    event e;
  endclass
  typedef struct {
    string s;
    int    v;
  } rec_t;

  event a, b, d, q, m, v, r, t, old_a, old_d, old_v, old_r, old_t;
  event ea[2], eq[$], em[int];
  evi i0 (), i1 ();
  virtual evi vif = i0;
  c h, h1 = new, h2 = new, o = new, o2 = new;
  string st, sa[2];
  rec_t rec;
  int x, calls;

  function automatic int f(int v);
    calls++;
    return v / 2;
  endfunction

  task automatic by_value(event e, string name);
    @(f(x) or e) $display("%0t %s", $time, name);
  endtask

  task automatic by_ref(ref event e, input string name);
    @(f(x) or e) $display("%0t %s", $time, name);
  endtask

  function string shown();
    return $sformatf("%b %s %s %s%0d", h == h1, st, sa[1], rec.s, rec.v);
  endfunction

  initial begin
    old_a = a;
    old_d = d;
    old_v = v;
    old_r = r;
    old_t = t;
    eq.push_back(q);
    em[3] = m;

    // Targets that a process of the assignment writes.
    h <= @a h1;
    st <= @a "s";
    sa[1] <= @(a or posedge x[8]) "e";
    rec <= repeat (2) @a '{"r", 1};
    a = b;
    #1 -> b;
    #1 $display("%0t other event: %s", $time, shown());
    -> old_a;
    #1 $display("%0t first: %s", $time, shown());
    a = null;
    -> old_a;
    #1 $display("%0t second: %s", $time, shown());
    a = old_a;

    // A list with a value that the waiting process evaluates.
    fork
      @(f(x) or d) $display("%0t d", $time);
      @(posedge f(x) or o.e) $display("%0t o.e", $time);
      @(f(x) or ea[x]) $display("%0t ea", $time);
      @(f(x) or eq[0]) $display("%0t eq", $time);
      @(f(x) or em[3]) $display("%0t em", $time);
      @(f(x) or vif.ie) $display("%0t vif", $time);
      by_value(v, "value");
      by_ref(r, "ref");
      begin
        h <= @(f(x) or o.e) h2;
        ->> @(f(x) or ea[0]) t;
        ->> repeat (2) @(f(x) or ea[0]) o2.e;
        t = b;
        @old_t $display("%0t t", $time);
        @o2.e $display("%0t o2.e", $time);
      end
    join_none
    #1;
    d = b;
    vif = i1;
    o.e = null;
    x = 1;
    eq[0] = b;
    em.delete(3);
    v = b;
    r = b;
    #1 -> i1.ie;
    #1 -> old_d;
    #1 -> i0.ie;
    #1 -> q;
    #1 -> m;
    #1 -> old_v;
    #1 -> old_r;
    #1 -> ea[1];
    #1 -> ea[0];
    #1 -> ea[0];
    #1 $display("%0t h %b", $time, h == h2);
    x = 2;
    #1 $display("%0t h %b", $time, h == h2);
    #1 $finish;
  end
endmodule
