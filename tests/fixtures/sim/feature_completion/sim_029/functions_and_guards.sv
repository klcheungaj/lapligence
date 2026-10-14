// SIM-029: functions in constraints (IEEE 1800-2009 18.5.11) and constraint
// guards (18.5.12).
class f_c;
  rand bit [3:0] x;
  rand bit [3:0] y;
  int calls;
  function bit [3:0] half(bit [3:0] v);
    calls++;
    return v >> 1;
  endfunction
  // y is an argument of half(), so y is solved first, then x <= half(y).
  constraint order_c { x <= half(y); }
  constraint y_c { y inside {2, 4, 8}; }
endclass

class node_c;
  int n;
  node_c next;
endclass

class list_c;
  rand int v;
  node_c head;
  constraint sort_c { if (head != null) v < head.n; v > -5; }
  constraint guard_c { head == null || head.n > 0 -> v != 3; }
endclass

module tb;
  initial begin
    static f_c f = new;
    static list_c l = new;
    int ok, i, bad, calls;

    bad = 0;
    for (i = 0; i < 50; i++) begin
      calls = f.calls;
      ok = f.randomize();
      if (ok != 1 || !(f.y inside {2, 4, 8}) || f.x > (f.y >> 1) || f.calls == calls) bad++;
    end
    $display("function ordering: bad=%0d", bad);

    bad = 0;
    for (i = 0; i < 30; i++) begin
      ok = l.randomize();                      // head is null: sort_c is guarded off
      if (ok != 1 || l.v <= -5 || l.v == 3) bad++;
    end
    $display("null guard: bad=%0d", bad);
    l.head = new;
    l.head.n = 2;
    bad = 0;
    for (i = 0; i < 30; i++) begin
      ok = l.randomize();
      if (ok != 1 || !(l.v > -5 && l.v < 2)) bad++;
    end
    $display("guard true: bad=%0d", bad);
    $finish;
  end
endmodule
