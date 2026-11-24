`include "defs.svh"
package class_pkg;
  class registry #(type T = int, string NAME = "none");
    static function T create(string name);
      return null;
    endfunction
  endclass
  typedef class item;
  class base_item;
    int total;
    function new(int total = 0);
      this.total = total;
    endfunction
    virtual function int step(int amount);
      return total + amount;
    endfunction
  endclass : base_item
  class item extends base_item;
    typedef registry #(item, "item") type_id;
    rand bit [1:0] level;
    constraint level_c {level < 2'd3;}
    extern function void run(item other);
    function new(int total = 0);
      super.new(.total(total));
    endfunction
  endclass : item
  class wrapper;
    item held;
    static int count;
  endclass
  function void item::run(item other);
    wrapper w;
    item made = item::type_id::create("made");
    w = new;
    w.held = made;
    wrapper::count = w.held.total;
    if (int'(other.step(total)) == 1) total = `CALL(other);
    if (!made.randomize() with {level == 2'd1;}) total = 0;
  endfunction
  // Nothing specializes this class.
  class pool #(type ELEMENT = base_item) extends base_item;
    ELEMENT members [$];
    function void add(ELEMENT member);
      members.push_back(member);
      total = members.size();
    endfunction
  endclass
endpackage
