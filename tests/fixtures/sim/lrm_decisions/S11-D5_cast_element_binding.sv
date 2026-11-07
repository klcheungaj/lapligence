// Decision S11-D5: a `$cast` into an element whose outcome depends on the run
// time binds the element first. For a class or enum destination whose source
// is not a constant or an upcast, the selectors run, then the source, and the
// missing associative entry or the queue element one past the end exists with
// the default value of its type whether or not the cast succeeds. An integral,
// real or string destination assigns as usual: source, then selectors. A
// constant enum source or an upcast also assigns as usual, so a failed
// constant cast creates nothing.
//
// IEEE 1800-2009 6.24.2 (SystemVerilog-1800-2009.txt L7433-7437): "When called
// as a function, $cast attempts to assign the source expression to the
// destination variable and returns 1 if the cast is legal. If the cast fails,
// the function does not make the assignment and returns 0." The text is
// silent on the order of the selector and the source, and on whether a failed
// cast creates the element it names; llg follows industry-standard behaviour.
class base;
  int a;
  function new(int x = 0); a = x; endfunction
endclass
class der extends base;
  function new(int x = 0); super.new(x); endfunction
endclass
class other extends base;
  function new(int x = 0); super.new(x); endfunction
endclass
typedef enum {R = 1, G = 2, B = 4} col_t;
module tb;
  function automatic int f(string s, int r);
    $display("f(%s)", s);
    return r;
  endfunction
  function automatic base mk(string s, int k);
    der d;
    other o;
    $display("mk(%s)", s);
    if (k == 0) return null;
    if (k == 1) begin d = new(7); return d; end
    o = new(8);
    return o;
  endfunction
  der q[$];
  der aa[int];
  col_t cq[$];
  col_t ca[string];
  int iq[$];
  int i = 0;
  function automatic base moves();
    i = 2;
    return mk("moves", 1);
  endfunction
  initial begin
    der t = new(1);
    der d3;
    q.push_back(t);
    q.push_back(t);
    q.push_back(t);
    // Selector, then source; the selector keeps the value it had.
    if ($cast(q[i], moves())) $display("%0d %0d %0d", q[0].a, q[1].a, q[2].a);
    if ($cast(q[f("sel", 1)], mk("src", 2))) $display("yes");
    else $display("no %0d", q[1].a);
    // The queue grows even though the cast fails.
    $display("%0d %0d", $cast(q[f("sel", 3)], mk("src", 2)), q.size());
    $display("%0d", q[3] == null);
    // So does an associative array.
    $display("%0d %0d %0d", $cast(aa[1], mk("src", 2)), aa.size(), aa.exists(1));
    // An enum destination binds the same way.
    cq.push_back(R);
    $display("%0d %0d", $cast(cq[1], f("val", 3)), cq.size());
    $display("%0d %0d", $cast(ca["a"], f("val", 3)), ca.size());
    // A constant source does not bind.
    $display("%0d %0d", $cast(ca["b"], 3), ca.size());
    // An integral destination evaluates its source first.
    iq.push_back(1);
    $display("%0d %0d", $cast(iq[f("sel", 0)], f("val", 5)), iq[0]);
    // An upcast is an assignment: a past-the-end write is dropped.
    d3 = t;
    $display("%0d %0d", $cast(q[9], d3), q.size());
  end
endmodule
