// Fixed arrays of class handles and strings assigned to dynamic arrays: the
// destination takes the source's size and a copy of each element, as a
// register model hands a broadcast callback its target fields.
class field;
  string name;
  function new(string name); this.name = name; endfunction
endclass

class broadcast_cb;
  field targets[];
  function string names();
    foreach (targets[i]) names = {names, " ", targets[i] == null ? "null" : targets[i].name};
  endfunction
endclass

class block;
  field f[3];
  broadcast_cb cb;
  function void build();
    foreach (f[i]) f[i] = new($sformatf("f%0d", i));
    cb = new;
    cb.targets = new[5];
    begin
      field t[3] = '{f[2], f[0], f[1]};
      cb.targets = t;
      t[0] = null;
    end
  endfunction
endclass

module tb;
  initial begin
    block b = new;
    string s[2] = '{"x", "y"};
    string d[];
    field g[2];
    field h[];
    b.build();
    $display("size %0d:%s", b.cb.targets.size(), b.cb.names());
    b.cb.targets = b.f;
    b.f[1] = null;
    $display("size %0d:%s", b.cb.targets.size(), b.cb.names());
    d = s;
    s[0] = "z";
    $display("%0d %s %s %s", d.size(), d[0], d[1], s[0]);
    h = g;
    $display("%0d %0d %0d", h.size(), h[0] == null, h[1] == null);
    begin
      string down[2:0] = '{"d2", "d1", "d0"};
      string up[1:3] = '{"u1", "u2", "u3"};
      d = down;
      $display("%0d %s %s %s", d.size(), d[0], d[1], d[2]);
      d = up;
      $display("%0d %s %s %s", d.size(), d[0], d[1], d[2]);
    end
  end
endmodule
