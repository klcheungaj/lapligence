// Decision S22-D7: a static class property's initializer runs once, before
// any procedure starts, whatever its value: a constant, another static
// property, a string expression or a constructed object. The property is
// shared by every object and by the class scope.
//
// IEEE 1800-2009 8.8 (SystemVerilog-1800-2009.txt L9870-9871, L9887,
// L9893):
//   "Sometimes only one version of a variable is required to be shared by all
//   instances. These class properties are created using the keyword static."
//   "Now, fileID shall be created and initialized once."
//   "The static class properties can be used without creating an object of
//   that type."
// IEEE 1800-2009 6.8 (L5286-5287): static class members are initialized
// "before any initial or always procedures are started".
module tb;
  class Seed;
    int v;
    function new(int x);
      v = x;
    endfunction
  endclass
  class Registry;
    static string name = "reg";
    static string tag = {name, "_tag"};
    static Seed seed = new(9);
    static int derived = seed.v + 1;
    static int created = 0;
    int id;
    function new();
      created++;
      id = created;
    endfunction
  endclass
  int seen = Registry::derived;
  initial begin
    Registry a, b;
    $display("before: %s %s derived=%0d seen=%0d created=%0d",
             Registry::name, Registry::tag, Registry::derived, seen, Registry::created);
    a = new;
    b = new;
    a.name = "renamed";
    $display("after: %s %s created=%0d ids=%0d,%0d", b.name, Registry::name,
             Registry::created, a.id, b.id);
    $finish;
  end
endmodule
