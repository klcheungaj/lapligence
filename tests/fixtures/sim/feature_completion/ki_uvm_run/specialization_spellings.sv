// One type reaches the specializations of `typeid` through two spellings:
// `typeid #(CB)` in `callbacks #(c0, c1)` and `typeid #(T)` in
// `callbacks #(c1, c2)` both name `typeid #(c1)` (8.25: matching parameters
// make one specialization, with one set of static members).
class typeid_base;
  static int created;
  function new();
    created++;
  endfunction
endclass

class typeid #(type T = int) extends typeid_base;
  static typeid #(T) inst;
  static function typeid #(T) get();
    if (inst == null) inst = new;
    return inst;
  endfunction
endclass

class c0; endclass
class c1; endclass
class c2; endclass
class c3; endclass
class c4; endclass
class c5; endclass
class c6; endclass
typedef c3 c3_alias;

class callbacks #(type T = int, type CB = int);
  static typeid_base ids[2];
  static function void add();
    ids[0] = typeid #(T)::get();
    ids[1] = typeid #(CB)::get();
  endfunction
endclass

module tb;
  initial begin
    callbacks #(c0, c1)::add();
    callbacks #(c1, c2)::add();
    callbacks #(c2, c3_alias)::add();
    callbacks #(c3, c4)::add();
    callbacks #(c4, c5)::add();
    callbacks #(c5, c6)::add();
    $display("c1 %0d", callbacks #(c0, c1)::ids[1] == callbacks #(c1, c2)::ids[0]);
    $display("c2 %0d", callbacks #(c1, c2)::ids[1] == callbacks #(c2, c3)::ids[0]);
    $display("c3 %0d", callbacks #(c2, c3)::ids[1] == callbacks #(c3, c4)::ids[0]);
    $display("c4 %0d", callbacks #(c3, c4)::ids[1] == callbacks #(c4, c5)::ids[0]);
    $display("c5 %0d", callbacks #(c4, c5)::ids[1] == callbacks #(c5, c6)::ids[0]);
    $display("c3 alias %0d", callbacks #(c2, c3_alias)::ids[1] == typeid #(c3)::get());
    $display("created %0d", typeid_base::created);
    $finish;
  end
endmodule
