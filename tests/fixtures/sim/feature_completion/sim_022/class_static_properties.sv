// SIM-022-A01: static class properties of string, handle, queue and integral
// type initialize once before time 0, whatever their initializer, and are
// shared by every object and the class scope.
module tb;
  class Seed;
    int v;
    function new(int x);
      v = x;
    endfunction
  endclass
  function automatic int seven();
    return 7;
  endfunction
  class Registry;
    static string name = "reg";
    static string tag = {name, "_tag"};
    static Seed seed = new(9);
    static int derived = seed.v + 1;
    static int called = seven();
    static int items[$] = '{1, 2};
    static int created = 0;
    int id;
    function new();
      created++;
      id = created;
      items.push_back(id);
    endfunction
  endclass
  class Derived extends Registry;
    static string label = {Registry::name, "_d"};
  endclass
  int seen = Registry::derived;
  initial begin
    Registry a, b;
    Derived d;
    $display("before: %s %s derived=%0d called=%0d seen=%0d items=%0d label=%s",
             Registry::name, Registry::tag, Registry::derived, Registry::called, seen,
             Registry::items.size(), Derived::label);
    a = new;
    b = new;
    d = new;
    a.name = "renamed";
    b.seed.v = 99;
    $display("after: %s %s seed=%0d created=%0d items=%0d ids=%0d,%0d,%0d", b.name,
             Registry::name, Registry::seed.v, Registry::created, d.items.size(), a.id, b.id, d.id);
    $finish;
  end
endmodule
