// FIN-001 G3 (F07-09, F07-14): properties of a computed class handle
// (a function result or a popped queue element) read, written and mutated;
// the handle expression is evaluated once per operation (SV 8.4, 11.4.1,
// 11.4.2, 7.10.2).
module tb;
  class C;
    int v;
    real r;
    string s;
    bit [7:0] b;
    C nxt;
  endclass
  class D extends C;
    int w;
  endclass
  C h = new, k = new;
  D dd = new;
  C q[$];
  int n, x, y;
  string t;
  function automatic C get();
    n++;
    return h;
  endfunction
  function automatic D getd();
    n++;
    return dd;
  endfunction
  initial begin
    k.v = 40;
    h.nxt = k;
    h.s = "hi";
    h.b = 8'h5a;
    get().v++;
    get().v += 5;
    ++get().v;
    get().v--;
    get().r += 1.5;
    getd().w += 2;
    getd().v -= 1;
    $display("a %0d %0.1f %0d %0d n %0d", h.v, h.r, dd.w, dd.v, n);
    x = get().v;
    y = get().v++;
    $display("b %0d %0d %0d n %0d", x, y, h.v, n);
    t = get().s;
    get().s = {t, "!"};
    get().v = get().nxt.v + 2;
    x = get().b[3:0];
    $display("c %s %0d %0d n %0d", h.s, h.v, x, n);
    q.push_back(k);
    q.push_back(h);
    q.push_back(dd);
    x = q.pop_front().v;
    $display("d %0d %0d %s %0d", x, q.size(), q.pop_front().s, q.size());
    q.push_back(h);
    q[1].v *= 2;
    $display("e %0d %0d", q.pop_back().v, dd.v);
    $finish(0);
  end
endmodule
