// Static class properties of record types with string, real and queue
// members (SV 8.9): one record shared by every object, initialized once in
// the static schedule, reached through the class scope, handles and methods.
typedef struct {
  string s;
  int n;
  real r;
} ns_t;
typedef struct {
  string s;
  int q[$];
} cq_t;

function automatic ns_t make(string s);
  make.s = s;
  make.n = s.len();
endfunction

class holder_c;
  static ns_t shared = '{s: "init", n: 1, r: 0.5};
  static ns_t plain;
  static ns_t made = make("abc");
  static cq_t bag;
  int id;
  function new(int i);
    id = i;
  endfunction
  function void bump();
    shared.n += id;
    shared.s = {shared.s, "+"};
  endfunction
  static function string tag();
    return $sformatf("%s/%0d", shared.s, shared.n);
  endfunction
  static function int count();
    return bag.q.size();
  endfunction
endclass

class child_c extends holder_c;
  function new();
    super.new(10);
  endfunction
  function void reset();
    shared = plain;
  endfunction
endclass

module tb;
  holder_c a, b;
  child_c c;
  ns_t copy;
  initial begin
    a = new(1);
    b = new(2);
    $display("1 %s %0d %0.2f", holder_c::shared.s, holder_c::shared.n, holder_c::shared.r);
    a.bump();
    b.bump();
    $display("2 %s %s", holder_c::tag(), a.shared.s);
    copy = holder_c::shared;
    c = new;
    c.bump();
    $display("3 %s %0d", c.tag(), copy.n);
    holder_c::shared.r = 2.25;
    $display("4 %0.2f %0d", b.shared.r, holder_c::shared == copy);
    c.reset();
    $display("5 [%s] %0d %0.2f", holder_c::shared.s, holder_c::shared.n, holder_c::shared.r);
    holder_c::bag.q.push_back(5);
    holder_c::bag.q.push_back(6);
    holder_c::bag.s = "b";
    $display("6 %s %0d %0d %0d %s", holder_c::made.s, holder_c::made.n, holder_c::count(),
             holder_c::bag.q[1], c.bag.s);
    $finish(0);
  end
endmodule
