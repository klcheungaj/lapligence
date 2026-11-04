// SV 13.5.2: module, static and block records as `ref` actuals of class methods, through direct, static, forwarded and virtual calls.
module tb;
  typedef struct { string s; int n; int a[3]; string t[2]; } rec_t;
  typedef struct { rec_t in; int k; } out_t;

  class K;
    int id = 1;
    function void set(ref rec_t x, input string s);
      x.s = {s, $sformatf("%0d", id)};
      x.n = x.n + id;
    endfunction
    function int peek(const ref rec_t x);
      return x.n + x.a[1] + id;
    endfunction
    function string tag(ref rec_t x);
      x.t[1] = "tag";
      return {x.t[1], x.s};
    endfunction
    task hold(ref rec_t x, input int dl);
      #dl;
      x.n = x.n * 2;
      $display("%0d hold n=%0d id=%0d", $time, x.n, id);
    endtask
    virtual function void vset(ref rec_t x);
      x.s = "K";
      x.n = 10;
    endfunction
    virtual task vhold(ref rec_t x, input int dl);
      #dl;
      x.s = "Kh";
      $display("%0d K vhold n=%0d", $time, x.n);
    endtask
    virtual function int vget(ref rec_t x);
      return x.n;
    endfunction
    function void forward(ref rec_t x);
      vset(x);
    endfunction
    static function void sset(ref rec_t x);
      x.s = "static";
    endfunction
  endclass

  class D extends K;
    function new; id = 2; endfunction
    virtual function void vset(ref rec_t x);
      super.vset(x);
      x.s = {x.s, "D"};
      x.n = x.n + id;
    endfunction
    virtual task vhold(ref rec_t x, input int dl);
      #dl;
      x.s = "Dh";
      super.vhold(x, 1);
      $display("%0d D vhold s=%s", $time, x.s);
    endtask
  endclass

  class E extends D;
    function new; id = 3; endfunction
  endclass

  rec_t mr, other;
  out_t o;
  K k;
  D d;
  E e;
  K h[3];

  task automatic twice(ref rec_t x, input K who);
    who.vset(x);
    who.set(x, "t");
  endtask

  initial begin
    static rec_t st;
    rec_t bk;
    k = new; d = new; e = new;
    h[0] = k; h[1] = d; h[2] = e;

    k.set(mr, "m");
    d.set(mr, "d");
    $display("mr %s %0d", mr.s, mr.n);
    mr.a[1] = 5;
    $display("peek %0d %0d", k.peek(mr), d.peek(mr));
    $display("tag %s %s", k.tag(mr), mr.t[1]);

    foreach (h[i]) begin
      h[i].vset(other);
      $display("vset[%0d] %s %0d vget %0d", i, other.s, other.n, h[i].vget(other));
    end

    k.forward(o.in);
    $display("forward %s %0d", o.in.s, o.in.n);
    d.forward(o.in);
    $display("forward %s %0d", o.in.s, o.in.n);

    d.set(st, "st");
    k.vset(bk);
    d.vset(st);
    $display("st %s %0d bk %s %0d", st.s, st.n, bk.s, bk.n);
    K::sset(bk);
    k.sset(st);
    $display("static %s %s", bk.s, st.s);

    twice(mr, d);
    $display("twice %s %0d", mr.s, mr.n);

    fork
      k.hold(mr, 2);
      begin #1 mr.n = 100; $display("%0d poke", $time); end
    join
    $display("n %0d", mr.n);

    fork
      h[1].vhold(mr, 2);
      h[0].vhold(other, 1);
      begin #1 mr.n = 7; other.n = 8; end
    join
    $display("%s %0d %s %0d", mr.s, mr.n, other.s, other.n);
    $finish;
  end
endmodule
