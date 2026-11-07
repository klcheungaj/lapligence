// 7.8.2: a nested string key taken from a function result, in a method.
class o;
  string nm;
  function new(string n); nm = n; endfunction
  function string get_name(); return nm; endfunction
endclass
typedef struct {
  int count;
  int cells[2];
  string label;
} entry_t;
class c;
  o m[string][string];
  entry_t e[string];
  function int fill(o x);
    e[x.get_name()].count = 3;
    e[x.get_name()].cells[1] = 4;
    e[x.get_name()].label = x.get_name();
    return e[x.get_name()].count + e[x.get_name()].cells[1];
  endfunction
  function bit put(o x, o y);
    m[x.get_name()][y.get_name()] = y;
    if (!m.exists(x.get_name()) || !m[x.get_name()].exists(y.get_name())) return 0;
    if (m[x.get_name()][y.get_name()] != y) return 0;
    $display("size %0d", m[x.get_name()].size());
    m[x.get_name()].delete(y.get_name());
    if (m[x.get_name()].size() == 0) m.delete(x.get_name());
    $display("left %0d", m.size());
    return 1;
  endfunction
endclass
module tb;
  initial begin
    automatic c k = new; automatic o a = new("a"), b = new("b");
    $display("%0d", k.put(a, b));
    $display("%0d %s", k.fill(a), k.e["a"].label);
  end
endmodule
