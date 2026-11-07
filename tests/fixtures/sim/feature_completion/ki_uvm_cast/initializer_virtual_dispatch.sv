// 10.5 and 8.20: a static class property initializer calls a function that
// dispatches virtual methods; every override of those slots is a function.
virtual class named;
  pure virtual function string name();
  virtual function void note(string text); $display("note %s %s", name(), text); endfunction
endclass
class leaf extends named;
  virtual function string name(); return "leaf"; endfunction
  static function leaf make();
    make = new;
  endfunction
endclass
class other extends named;
  virtual function string name(); return "other"; endfunction
  virtual function void note(string text); $display("other %s", text); endfunction
  static function other make();
    make = new;
  endfunction
endclass
class registry;
  static named items[$];
  static function named add(named item);
    item.note("added");
    items.push_back(item);
    return item;
  endfunction
endclass
class user;
  static named first = registry::add(leaf::make());
  static named second = registry::add(other::make());
endclass
module tb;
  initial $display("%s %s %0d", user::first.name(), user::second.name(), registry::items.size());
endmodule
